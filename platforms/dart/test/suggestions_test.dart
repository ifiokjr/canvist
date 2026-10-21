/// Mentions, hashtags, and emoji through the Dart API: typed handles,
/// engines without them, error codes, and two peers editing them together.
library;

import 'dart:typed_data';

import 'package:canvist/canvist.dart';
import 'package:test/test.dart';

typedef Atoms = ({
  Engine engine,
  MentionHandle mention,
  HashtagHandle hashtag,
  EmojiHandle emoji,
});

Future<Atoms> atoms({int client = 1}) async {
  final builder = EngineBuilder();
  final mention = builder.add(Extensions.mention);
  final hashtag = builder.add(Extensions.hashtag);
  final emoji = builder.add(Extensions.emoji);
  final engine = await builder.build(client: client);

  return (engine: engine, mention: mention, hashtag: hashtag, emoji: emoji);
}

Matcher throwsCode(ErrorCode code) => throwsA(
  isA<CanvistException>().having((error) => error.code, 'code', code),
);

/// Deliver each peer's pending updates to the other.
void sync(Doc first, Doc second) {
  final List<Uint8List> fromFirst = first.takeUpdates();
  final List<Uint8List> fromSecond = second.takeUpdates();
  fromFirst.forEach(second.applyUpdate);
  fromSecond.forEach(first.applyUpdate);
}

void main() {
  test('typed handles suggest, insert, and dismiss atoms', () async {
    final (:engine, :mention, :hashtag, :emoji) = await atoms();
    final doc = engine.create()..typeText('Hi @ad');

    expect(doc.query(mention.mentionSuggestion())?.query, 'ad');
    expect(doc.can(mention.insertMention(id: '7')), isTrue);
    expect(doc.run(mention.insertMention(id: '7', label: 'Ada')), isTrue);
    expect(doc.query(mention.mentionSuggestion()), isNull);
    expect(
      doc.selection?.head,
      const Position([0], 5),
      reason: 'an atom is one UTF-16 unit',
    );

    doc.typeText(':thu');
    expect(doc.query(emoji.emojiSuggestion())?.char, ':');
    final found = doc.query(emoji.searchEmoji(query: 'thumbs'));
    expect(found.map((item) => item.name), ['thumbsdown', 'thumbsup']);
    expect(doc.query(emoji.searchEmoji(query: 'face')), hasLength(10));
    expect(doc.query(emoji.searchEmoji(query: 'face', limit: 0)), isEmpty);
    expect(doc.run(emoji.setEmoji(name: 'nope')), isFalse);
    expect(doc.run(emoji.setEmoji(name: 'thumbsup')), isTrue);

    doc.typeText('#ca');
    expect(doc.query(hashtag.hashtagSuggestion())?.query, 'ca');
    expect(doc.run(hashtag.dismissHashtagSuggestion()), isTrue);
    expect(doc.run(hashtag.dismissHashtagSuggestion()), isFalse);
    doc.typeText(' ');
    expect(doc.query(hashtag.hashtags()), [
      'ca',
    ], reason: 'a dismissed suggestion is still a tag once its word ends');
    doc.run(hashtag.insertHashtag(tag: '#canvas'));
    expect(doc.query(hashtag.hashtags()), ['ca', 'canvas']);
    expect(doc.export(Format.text), 'Hi @Ada 👍 #ca #canvas ');

    doc.typeText(':smile:');
    expect(doc.export(Format.text), endsWith('😄'));
    doc.press('Backspace');
    expect(
      doc.export(Format.text),
      endsWith(':smile:'),
      reason: 'Backspace undoes the conversion',
    );
  });

  test('undo and redo restore suggestions and atoms', () async {
    final (:engine, :mention, hashtag: _, emoji: _) = await atoms();
    final doc = engine.create()..typeText('@ad');
    doc.run(mention.insertMention(id: '1', label: 'Ann'));

    expect(doc.undo(), isTrue);
    expect(doc.export(Format.text), '@ad');
    expect(doc.query(mention.mentionSuggestion())?.query, 'ad');
    expect(doc.redo(), isTrue);
    expect(doc.export(Format.text), '@Ann ');
  });

  test('invalid arguments and missing extensions have stable codes', () async {
    final (:engine, mention: _, hashtag: _, emoji: _) = await atoms();
    final doc = engine.create()..typeText('@a');
    final before = doc.export(Format.json);

    expect(
      () => doc.runNamed('insertMention'),
      throwsCode(ErrorCode.invalidArgs),
    );
    expect(
      () => doc.runNamed('insertMention', {'id': 7}),
      throwsCode(ErrorCode.invalidArgs),
    );
    expect(
      () => doc.queryNamed('searchEmoji', {'query': 'a', 'limit': 'ten'}),
      throwsCode(ErrorCode.invalidArgs),
    );
    expect(doc.export(Format.json), before);

    final (engine: plain, kit: _) = await Engine.starterKit();
    expect(
      () => plain.create().runNamed('setEmoji', {'name': 'smile'}),
      throwsCode(ErrorCode.unknownName),
    );

    final other = await atoms();
    expect(
      () => plain.create().run(other.mention.insertMention(id: '1')),
      throwsArgumentError,
      reason: 'a handle from another engine is refused',
    );
  });

  test('TipTap JSON atoms load and export as TipTap HTML', () async {
    final (:engine, :hashtag, mention: _, emoji: _) = await atoms();
    final doc = engine.fromJson({
      'type': 'doc',
      'content': [
        {
          'type': 'paragraph',
          'content': [
            {
              'type': 'mention',
              'attrs': {'id': 'u1'},
            },
            {'type': 'text', 'text': ' '},
            {
              'type': 'emoji',
              'attrs': {'name': 'smile'},
            },
            {
              'type': 'text',
              'text': ' #x',
              'marks': [
                {
                  'type': 'hashtag',
                  'attrs': {'tag': 'x'},
                },
              ],
            },
          ],
        },
      ],
    });

    expect(doc.export(Format.text), '@u1 :smile: #x');
    expect(doc.query(hashtag.hashtags()), ['x']);
    expect(
      doc.export(Format.html),
      '<p><span data-type="mention" data-id="u1">@u1</span> '
      '<span data-type="emoji" data-name="smile">:smile:</span>'
      '<span data-type="hashtag" data-tag="x"> #x</span></p>',
    );
  });

  test('two peers adding atoms concurrently converge', () async {
    final a = await atoms();
    final b = await atoms(client: 2);
    final first = a.engine.parse(Format.text, 'Hello world')..share(1);
    final second = b.engine.fromYjs(first.encodeState(), clientId: 2);

    first
      ..select(const Position([0], 5))
      ..typeText(' @gr')
      ..run(a.mention.insertMention(id: '3', label: 'Grace'));
    second
      ..select(const Position([0], 5))
      ..typeText(' :rocket: #wasm ');
    sync(first, second);

    expect(first.export(Format.html), second.export(Format.html));
    expect(first.toJson()['content'], second.toJson()['content']);
    expect(first.export(Format.html), contains('data-label="Grace"'));
    expect(first.export(Format.html), contains('data-name="rocket"'));
    expect(second.query(b.hashtag.hashtags()), ['wasm']);
    expect(second.validate().rejectedBlocks, 0);

    expect(first.undo(), isTrue);
    sync(first, second);
    expect(second.export(Format.html), isNot(contains('data-type="mention"')));
    expect(first.export(Format.html), second.export(Format.html));
  });
}
