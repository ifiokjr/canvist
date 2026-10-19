/// The `dart:io` example server end to end: HTTP conversion, and two peers
/// editing one room over WebSockets with the y-sync protocol.
library;

import 'dart:async';
import 'dart:convert';
import 'dart:io';
import 'dart:typed_data';

import 'package:canvist/canvist.dart';
import 'package:test/test.dart';

import '../example/server.dart';

/// y-sync's message type for sync messages, and its step-2 subtype.
const _messageSync = 0;
const _syncStep2 = 1;

/// Sync step 1 with an empty state vector: "send me everything".
final _askForEverything = Uint8List.fromList([_messageSync, 0, 1, 0]);

/// A peer that joins a room the way y-websocket clients do, holding its
/// copy of the document in a canvist [Doc].
final class Peer {
  Peer._(this._socket, this._engine, this._clientId);

  final WebSocket _socket;
  final Engine _engine;
  final int _clientId;
  Doc? doc;

  static Future<Peer> connect(Uri uri, Engine engine, int clientId) async {
    final peer = Peer._(await WebSocket.connect('$uri'), engine, clientId);
    peer._socket.listen(
      (data) => peer._receive(Uint8List.fromList(data as List<int>)),
    );
    peer._socket.add(_askForEverything);

    return peer;
  }

  void _receive(Uint8List message) {
    final doc = this.doc;

    if (doc != null) {
      doc.receiveSync(message).reply.forEach(_socket.add);

      return;
    }

    // Before joining, only the server's answer to "everything" matters.
    if (message[0] == _messageSync && message[1] == _syncStep2) {
      this.doc = _engine.fromYjs(_payload(message), clientId: _clientId);
    }
  }

  /// Type [text] at the start of top-level block [block] and send it.
  void type(int block, String text) {
    final doc = this.doc!..select(Position([block], 0));
    doc.insertText(text);
    doc.flushSync().forEach(_socket.add);
  }

  Future<void> close() => _socket.close();
}

/// The update in a sync step 2 message: `[0, 1, length, ...bytes]` with a
/// lib0 varint length.
Uint8List _payload(Uint8List message) {
  var offset = 2;
  var length = 0;
  var shift = 0;

  while (true) {
    final byte = message[offset++];
    length |= (byte & 0x7f) << shift;
    shift += 7;

    if (byte < 0x80) {
      break;
    }
  }

  return Uint8List.sublistView(message, offset, offset + length);
}

Future<void> _until(bool Function() condition, String what) async {
  final deadline = DateTime.now().add(const Duration(seconds: 5));

  while (!condition()) {
    if (DateTime.now().isAfter(deadline)) {
      fail('timed out waiting for $what');
    }

    await Future<void>.delayed(const Duration(milliseconds: 10));
  }
}

void main() {
  late HttpServer server;
  late Engine engine;

  setUpAll(() async {
    server = await serve(port: 0);
    engine = (await Engine.starterKit(client: 9)).engine;
  });

  tearDownAll(() => server.close(force: true));

  Uri at(String path, [String scheme = 'http']) =>
      Uri.parse('$scheme://localhost:${server.port}$path');

  test('converts between formats over HTTP', () async {
    final client = HttpClient();
    addTearDown(client.close);

    Future<(int, String)> post(String path, String body) async {
      final request = await client.postUrl(at(path))
        ..write(body);
      final response = await request.close();

      return (response.statusCode, await utf8.decodeStream(response));
    }

    final (status, json) = await post('/convert?from=markdown&to=json', '# Hi');
    expect(status, 200);
    expect(
      ((jsonDecode(json) as Map<String, Object?>)['content']! as List<Object?>)
          .length,
      1,
    );

    final (unknown, _) = await post('/convert?from=docx', '');
    expect(unknown, 400);
  });

  test('peers edit one room through the y-sync endpoint', () async {
    final alice = await Peer.connect(at('/sync/notes', 'ws'), engine, 21);
    final bob = await Peer.connect(at('/sync/notes', 'ws'), engine, 22);
    addTearDown(alice.close);
    addTearDown(bob.close);

    await _until(
      () => alice.doc != null && bob.doc != null,
      'both peers to join',
    );
    expect(bob.doc!.export(Format.text), 'Shared notes\nEdit me together.');

    alice.type(1, 'Alice says hi. ');
    await _until(
      () => bob.doc!.export(Format.text).contains('Alice says hi.'),
      "bob to see alice's edit",
    );

    final client = HttpClient();
    addTearDown(client.close);
    final response = await (await client.getUrl(
      at('/rooms/notes?format=markdown'),
    )).close();
    expect(
      await utf8.decodeStream(response),
      '# Shared notes\n\nAlice says hi. Edit me together.',
    );
  });
}
