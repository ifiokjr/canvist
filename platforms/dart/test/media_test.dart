/// Media blocks through the Dart API: typed handles, refused video URLs,
/// error codes, the compiled-in emoji table, and two peers inserting media
/// together.
library;

import 'dart:typed_data';

import 'package:canvist/canvist.dart';
import 'package:test/test.dart';

typedef Media = ({
  Engine engine,
  ImageHandle image,
  YoutubeHandle youtube,
  TwitchHandle twitch,
  AudioHandle audio,
  FileHandle file,
});

Future<Media> media({int client = 1}) async {
  final builder = EngineBuilder();
  final image = builder.add(Extensions.image);
  final youtube = builder.add(Extensions.youtube);
  final twitch = builder.add(Extensions.twitch);
  final audio = builder.add(Extensions.audio);
  final file = builder.add(Extensions.file);
  final engine = await builder.build(client: client);

  return (
    engine: engine,
    image: image,
    youtube: youtube,
    twitch: twitch,
    audio: audio,
    file: file,
  );
}

Matcher throwsCode(ErrorCode code) => throwsA(
  isA<CanvistException>().having((error) => error.code, 'code', code),
);

/// Block types and attributes, without block ids.
List<Object?> blocks(Doc doc) => [
  for (final block in doc.toJson()['content']! as List<Object?>)
    {
      for (final entry in (block! as Map<String, Object?>).entries)
        if (entry.key == 'attrs')
          'attrs': {
            for (final attr in (entry.value! as Map<String, Object?>).entries)
              if (attr.key != 'id') attr.key: attr.value,
          }
        else
          entry.key: entry.value,
    },
];

void sync(Doc first, Doc second) {
  final List<Uint8List> fromFirst = first.takeUpdates();
  final List<Uint8List> fromSecond = second.takeUpdates();
  fromFirst.forEach(second.applyUpdate);
  fromSecond.forEach(first.applyUpdate);
}

void main() {
  test('typed handles insert every media block', () async {
    final m = await media();
    final doc = m.engine.parse(Format.text, 'ab')
      ..select(const Position([0], 1));

    expect(doc.can(m.image.setImage(src: 'a.png')), isTrue);
    expect(
      doc.run(m.image.setImage(src: 'a.png', alt: 'A', width: 320.5)),
      isTrue,
    );
    expect(doc.selection?.head, const Position([2], 0));
    expect(
      doc.run(m.youtube.setYoutubeVideo(src: 'https://youtu.be/x', start: 30)),
      isTrue,
    );
    expect(
      doc.run(m.twitch.setTwitchVideo(src: 'https://twitch.tv/a', muted: true)),
      isTrue,
    );
    expect(doc.run(m.audio.setAudio(src: 'a.mp3', controls: false)), isTrue);
    expect(
      doc.run(m.file.insertFile(url: 'r.pdf', fileName: 'Report', fileSize: 9)),
      isTrue,
    );

    expect(blocks(doc).map((block) => (block! as Map)['type']), [
      'paragraph',
      'image',
      'youtube',
      'twitch',
      'audio',
      'file',
      'paragraph',
    ]);
    expect((blocks(doc)[1]! as Map)['attrs'], {
      'alt': 'A',
      'src': 'a.png',
      'width': 320.5,
    });
    expect(
      doc.export(Format.html),
      contains('<iframe title="YouTube video" src="https://youtu.be/x">'),
    );
    expect(doc.export(Format.markdown), endsWith('[Report](r.pdf)\n\nb'));

    expect(doc.undo(), isTrue);
    expect(doc.export(Format.html), isNot(contains('data-type="file"')));
    expect(doc.redo(), isTrue);
    expect(doc.export(Format.html), contains('data-type="file"'));
  });

  test('video handles refuse URLs from other sites', () async {
    final m = await media();
    final doc = m.engine.create();

    for (final src in [
      "javascript:alert('youtube')",
      'https://youtube.com.example.com/x',
      'youtube.com/x',
    ]) {
      expect(
        doc.can(m.youtube.setYoutubeVideo(src: src)),
        isFalse,
        reason: src,
      );
      expect(
        doc.run(m.youtube.setYoutubeVideo(src: src)),
        isFalse,
        reason: src,
      );
    }

    expect(
      doc.run(m.twitch.setTwitchVideo(src: 'https://youtu.be/x')),
      isFalse,
    );
    expect(doc.export(Format.html), '<p></p>');
  });

  test('invalid media arguments have stable codes', () async {
    final m = await media();
    final doc = m.engine.create();
    final before = doc.export(Format.json);

    expect(
      () => doc.runNamed('setImage', {'src': null}),
      throwsCode(ErrorCode.invalidArgs),
      reason: 'the typed handle requires a source; Rust refuses a null one',
    );
    expect(
      () => doc.runNamed('setImage', {'src': 'a.png', 'width': 'wide'}),
      throwsCode(ErrorCode.invalidArgs),
    );
    expect(
      () => doc.runNamed('insertFile', {'fileName': 'r'}),
      throwsCode(ErrorCode.invalidArgs),
    );
    expect(doc.export(Format.json), before);

    final (:engine, kit: _) = await Engine.starterKit();
    expect(
      () => engine.create().runNamed('setImage', {'src': 'a.png'}),
      throwsCode(ErrorCode.unknownName),
    );
    expect(
      () => engine.create().run(m.image.setImage(src: 'a.png')),
      throwsArgumentError,
      reason: 'a handle from another engine is refused',
    );
  });

  test('the Dart package compiles the emoji table in', () async {
    final builder = EngineBuilder();
    final emoji = builder.add(Extensions.emoji);
    final engine = await builder.build();
    final doc = engine.create()..typeText('ship :rocket: ');

    expect(doc.export(Format.text), 'ship 🚀 ');
    expect(
      doc.query(emoji.searchEmoji(query: 'rocke', limit: 1)).single.emoji,
      '🚀',
    );
  });

  test('two peers inserting media concurrently converge', () async {
    final a = await media();
    final b = await media(client: 2);
    final first = a.engine.parse(Format.text, 'one\ntwo')..share(1);
    final second = b.engine.fromYjs(first.encodeState(), clientId: 2);

    first
      ..select(const Position([0], 3))
      ..run(a.image.setImage(src: 'a.png', width: 320.5));
    second
      ..select(const Position([0], 3))
      ..run(b.audio.setAudio(src: 'a.mp3', loop: true));
    sync(first, second);

    expect(blocks(first), blocks(second));
    expect(first.export(Format.html), second.export(Format.html));
    expect(
      first.export(Format.html),
      allOf(
        contains('<img src="a.png" width="320.5">'),
        contains('<audio src="a.mp3" controls loop>'),
      ),
    );
    expect(second.validate().rejectedBlocks, 0);

    expect(first.undo(), isTrue);
    sync(first, second);
    expect(second.export(Format.html), isNot(contains('<img')));
    expect(blocks(first), blocks(second));
  });
}
