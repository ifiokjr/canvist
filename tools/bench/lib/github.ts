/**
 * The GitHub Actions surface: the job summary, step outputs, and one sticky
 * comment per check on the pull request.
 *
 * @module
 */
import { appendFileSync, readFileSync } from "node:fs";

const HTTP_FORBIDDEN = 403;
const HTTP_NOT_FOUND = 404;
const COMMENTS_PER_PAGE = 100;

/** Appends Markdown to the job summary, or prints it outside Actions. */
export function writeSummary(markdown: string): void {
	const path = process.env["GITHUB_STEP_SUMMARY"];

	if (path === undefined) {
		console.log(markdown);

		return;
	}

	appendFileSync(path, `${markdown}\n`);
}

/** Sets a step output, or prints it outside Actions. */
export function setOutput(name: string, value: string): void {
	const path = process.env["GITHUB_OUTPUT"];

	if (path === undefined) {
		console.log(`${name}=${value}`);

		return;
	}

	appendFileSync(path, `${name}=${value}\n`);
}

interface PullRequestEvent {
	pull_request?: { number: number; labels?: { name: string }[] };
}

function readEvent(): PullRequestEvent {
	const path = process.env["GITHUB_EVENT_PATH"];

	return path === undefined
		? {}
		: JSON.parse(readFileSync(path, "utf8")) as PullRequestEvent;
}

/** The labels on the pull request that started this run. */
export function pullRequestLabels(): string[] {
	return readEvent().pull_request?.labels?.map((label) => label.name) ?? [];
}

async function request(
	path: string,
	init: { method?: string; body?: string } = {},
): Promise<Response> {
	const api = process.env["GITHUB_API_URL"] ?? "https://api.github.com";
	const headers = {
		accept: "application/vnd.github+json",
		authorization: `Bearer ${process.env["GITHUB_TOKEN"] ?? ""}`,
		"content-type": "application/json",
	};

	return await fetch(`${api}${path}`, { ...init, headers });
}

/**
 * Creates or updates this check's comment on the pull request, found by the
 * hidden `marker` it starts with.
 *
 * A pull request from a fork gets a read-only token and can't comment; the
 * job summary still has the report, so that is a warning, not a failure.
 */
export async function upsertComment(
	marker: string,
	markdown: string,
): Promise<void> {
	const number = readEvent().pull_request?.number;
	const repository = process.env["GITHUB_REPOSITORY"];

	if (number === undefined || repository === undefined) return;

	const tag = `<!-- canvist-bench:${marker} -->`;
	const body = JSON.stringify({ body: `${tag}\n${markdown}` });
	const list = await request(
		`/repos/${repository}/issues/${number}/comments?per_page=${COMMENTS_PER_PAGE}`,
	);
	const comments = list.ok
		? await list.json() as { id: number; body?: string }[]
		: [];
	const existing = comments.find((comment) => comment.body?.startsWith(tag));
	const response = existing === undefined
		? await request(`/repos/${repository}/issues/${number}/comments`, {
			method: "POST",
			body,
		})
		: await request(`/repos/${repository}/issues/comments/${existing.id}`, {
			method: "PATCH",
			body,
		});

	if (
		response.status === HTTP_FORBIDDEN || response.status === HTTP_NOT_FOUND
	) {
		console.log(
			`::warning::could not comment on the pull request (${response.status})`,
		);

		return;
	}

	if (!response.ok) {
		throw new Error(
			`commenting failed: ${response.status} ${await response.text()}`,
		);
	}
}
