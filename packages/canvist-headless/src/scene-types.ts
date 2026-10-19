/**
 * The scene JSON format: canvist canvas documents of pages, frames, shapes,
 * images, vector paths, and text frames.
 *
 * These types mirror `canvist_scene`'s JSON codec. A scene can be written
 * nested, with `children` and without ids or keys, which is how people and
 * agents write one; {@link SceneDoc.toJson} returns the canonical flat form,
 * where every node names its `parent` and `key`.
 *
 * @module
 */
import type { Registered, StarterKit } from "./engine.ts";
import type { DocJson } from "./types.ts";

/** A colour as `#rrggbb` or `#rrggbbaa`. */
export type HexColor = `#${string}`;

/** A node id, `client.seq` in base 36, such as `"1.a"`. */
export type NodeId = string;

/** A 2D point or vector: `[x, y]`. */
export type Vec2 = readonly [number, number];

/** An affine transform's coefficients: `[a, b, c, d, e, f]`. */
export type Affine = readonly [number, number, number, number, number, number];

/** How a paint or layer blends with what is below it. */
export type BlendMode =
	| "normal"
	| "multiply"
	| "screen"
	| "overlay"
	| "darken"
	| "lighten"
	| "color-dodge"
	| "color-burn"
	| "hard-light"
	| "soft-light"
	| "difference"
	| "exclusion"
	| "hue"
	| "saturation"
	| "color"
	| "luminosity";

/** One stop of a gradient. */
export interface GradientStop {
	/** Where the stop sits, 0 to 1. */
	readonly offset: number;
	readonly color: HexColor;
}

/** Fields every paint may have. */
interface PaintCommon {
	/** 0 to 1. Defaults to 1. */
	readonly opacity?: number;
	/** Defaults to `true`. */
	readonly visible?: boolean;
	/** Defaults to `"normal"`. */
	readonly blend?: BlendMode;
}

/** A fill or stroke. Gradient points are in the node's unit box. */
export type PaintJson =
	& PaintCommon
	& (
		| { readonly type: "solid"; readonly color: HexColor }
		| {
			readonly type: "linear";
			readonly from: Vec2;
			readonly to: Vec2;
			readonly stops: readonly GradientStop[];
		}
		| {
			readonly type: "radial";
			readonly center: Vec2;
			readonly radius: Vec2;
			readonly stops: readonly GradientStop[];
		}
		| {
			readonly type: "sweep";
			readonly center: Vec2;
			/** Degrees clockwise from the positive x axis. */
			readonly angle: number;
			readonly stops: readonly GradientStop[];
		}
		| {
			readonly type: "image";
			/** The host's id for the image, such as a URL or content hash. */
			readonly asset: string;
			/** Defaults to `"fill"`. */
			readonly fit?: "fill" | "fit" | "stretch" | "tile";
		}
	);

/** How a node's strokes are drawn. Every field has a default. */
export interface StrokeJson {
	readonly width?: number;
	readonly align?: "inside" | "center" | "outside";
	readonly cap?: "butt" | "round" | "square";
	readonly join?: "miter" | "round" | "bevel";
	readonly miterLimit?: number;
	/** Alternating dash and gap lengths; empty for a solid line. */
	readonly dashes?: readonly number[];
}

/** A shadow's colour, offset, blur, and spread. */
interface ShadowJson {
	readonly color: HexColor;
	readonly offset?: Vec2;
	readonly blur?: number;
	readonly spread?: number;
}

/** A shadow or blur on a node. */
export type EffectJson =
	& { readonly visible?: boolean }
	& (
		| ({ readonly type: "drop-shadow" } & ShadowJson)
		| ({ readonly type: "inner-shadow" } & ShadowJson)
		| { readonly type: "layer-blur"; readonly radius: number }
		| { readonly type: "background-blur"; readonly radius: number }
	);

/** A node's own geometry. Most node types imply theirs. */
export type ShapeJson =
	| { readonly type: "none" }
	| { readonly type: "rect" }
	| {
		readonly type: "ellipse";
		/** Arc start in degrees. */
		readonly start?: number;
		/** Arc sweep in degrees; 360 for a full ellipse. */
		readonly sweep?: number;
		/** Inner radius as a fraction of the outer; 0 for a disc. */
		readonly inner?: number;
	}
	| {
		readonly type: "polygon";
		/** Outer points, at least 3. */
		readonly points?: number;
		/** Inner radius as a fraction of the outer; 1 for a plain polygon. */
		readonly ratio?: number;
	}
	| {
		readonly type: "path";
		/** SVG path data in the node's own coordinates. */
		readonly d: string;
		readonly rule?: "nonzero" | "evenodd";
	}
	| {
		readonly type: "boolean";
		readonly op: "union" | "subtract" | "intersect" | "exclude";
	}
	| {
		readonly type: "text";
		readonly sizing?: "fixed" | "auto-height" | "auto-width";
	}
	| { readonly type: "instance"; readonly component: NodeId };

/** How a node follows its parent frame when the frame is resized. */
export type Constraint = "start" | "end" | "both" | "center" | "scale";

/** A frame's auto-layout. */
export interface AutoLayoutJson {
	readonly direction?: "row" | "column";
	readonly gap?: number;
	/** One number for every side, or `[top, right, bottom, left]`. */
	readonly padding?: number | readonly [number, number, number, number];
	readonly align?: Align;
	readonly crossAlign?: Align;
	readonly wrap?: boolean;
}

/** How auto-layout places children along an axis. */
export type Align = "start" | "center" | "end" | "space-between" | "stretch";

/** How a child of an auto-layout frame is sized on each axis. */
export type Sizing = "fixed" | "fill" | "hug";

/**
 * A node's properties. Every one is optional and defaults by node type, so
 * a node lists only what differs. Custom properties are `c:<name>`, and an
 * instance's overrides are `o:<node id>:<property>`.
 */
export interface PropsJson {
	readonly name?: string;
	readonly visible?: boolean;
	readonly locked?: boolean;
	/** Position, rotation, and skew; size lives in `size`. */
	readonly transform?: Affine;
	readonly size?: Vec2;
	readonly opacity?: number;
	readonly blend?: BlendMode;
	readonly fills?: readonly PaintJson[];
	readonly strokes?: readonly PaintJson[];
	readonly stroke?: StrokeJson;
	readonly effects?: readonly EffectJson[];
	/** Corner radii: top left, top right, bottom right, bottom left. */
	readonly radius?: readonly [number, number, number, number];
	/** Whether a frame clips its children. */
	readonly clip?: boolean;
	readonly constraints?: {
		readonly horizontal?: Constraint;
		readonly vertical?: Constraint;
	};
	/** A frame's auto-layout, or `null` for none. */
	readonly layout?: AutoLayoutJson | null;
	readonly layoutChild?: {
		readonly width?: Sizing;
		readonly height?: Sizing;
		readonly absolute?: boolean;
	};
	readonly shape?: ShapeJson;
	readonly [custom: `c:${string}`]: unknown;
	readonly [override: `o:${string}`]: unknown;
}

/** The built-in node types. */
export type NodeType =
	| "page"
	| "frame"
	| "group"
	| "rectangle"
	| "ellipse"
	| "polygon"
	| "path"
	| "boolean"
	| "image"
	| "text"
	| "component"
	| "instance";

/** A text frame's document: StarterKit ProseMirror JSON. */
export type TextJson = DocJson<Registered<StarterKit>>;

/** Fields every node has, in either form. */
interface NodeCommon {
	readonly type: NodeType;
	readonly props?: PropsJson;
	/** For text frames. */
	readonly text?: TextJson;
}

/** A node in the nested form: ids and keys optional, children inline. */
export interface NestedNodeJson extends NodeCommon {
	readonly id?: NodeId;
	/** A fractional key among its siblings; list order when omitted. */
	readonly key?: string;
	readonly children?: readonly NestedNodeJson[];
}

/** A node in the canonical flat form {@link SceneDoc.toJson} writes. */
export interface FlatNodeJson extends NodeCommon {
	readonly id: NodeId;
	/** The parent's id, or `"root"` for a page. */
	readonly parent: NodeId;
	readonly key: string;
}

/** A scene JSON document. */
export interface SceneJson<
	N extends NestedNodeJson | FlatNodeJson = NestedNodeJson | FlatNodeJson,
> {
	readonly type: "canvist-scene";
	readonly version: 1;
	readonly nodes: readonly N[];
}

/** What an export draws. Every field is optional. */
export interface ExportOptions {
	/** The nodes to draw, with everything inside them. Omit for the first page with its background. */
	readonly nodes?: readonly NodeId[];
	/** Output pixels per world unit. Defaults to 1. */
	readonly scale?: number;
	/** Space around the drawing, in world units. Defaults to 0. */
	readonly padding?: number;
}
