/// An example canvist server on `dart:io`, matching the TypeScript one:
///
/// - `POST /convert?from=markdown&to=json` converts the request body.
/// - `GET /rooms/<name>` returns a room's document (`?format=` picks one).
/// - `/sync/<name>` is a y-websocket-compatible WebSocket endpoint: plain
///   Yjs clients (`WebsocketProvider`) edit one shared document per room,
///   which the server keeps schema-checked and readable at any time.
///
/// ```sh
/// dart run example/server.dart
/// curl -X POST 'localhost:8787/convert?from=markdown&to=html' -d '# Hello **world**'
/// ```
library;

import 'dart:async';
import 'dart:convert';
import 'dart:io';
import 'dart:typed_data';

import 'package:canvist/canvist.dart';

/// The yrs client id the server writes its own changes as.
const _serverClient = 1;

const _contentTypes = {
  Format.json: 'application/json',
  Format.html: 'text/html; charset=utf-8',
  Format.markdown: 'text/markdown; charset=utf-8',
  Format.text: 'text/plain; charset=utf-8',
};

/// One shared document and the connections editing it.
final class Room {
  Room(this.doc);

  final Doc doc;
  final _sockets = <WebSocket>{};

  void join(WebSocket socket) {
    _sockets.add(socket);
    socket.add(doc.syncStart());
  }

  void leave(WebSocket socket) => _sockets.remove(socket);

  void receive(WebSocket from, Uint8List message) {
    final sync = doc.receiveSync(message);
    sync.reply.forEach(from.add);

    for (final socket in _sockets.where((socket) => socket != from)) {
      sync.broadcast.forEach(socket.add);
    }
  }
}

/// Rooms by name, each seeded from [initial] Markdown and shared by the
/// server before anyone joins.
final class Rooms {
  Rooms(this._engine, this._initial);

  final Engine _engine;
  final String _initial;
  final _rooms = <String, Room>{};

  Room operator [](String name) => _rooms.putIfAbsent(name, () {
    final doc = _engine.parse(Format.markdown, _initial)..share(_serverClient);

    return Room(doc);
  });
}

/// Start the server; [port] 0 picks a free one.
Future<HttpServer> serve({int port = 8787}) async {
  final (:engine, kit: _) = await Engine.starterKit();
  final rooms = Rooms(engine, '# Shared notes\n\nEdit me together.');
  final server = await HttpServer.bind(InternetAddress.loopbackIPv4, port);
  server.listen((request) => unawaited(_handle(request, engine, rooms)));

  return server;
}

Future<void> _handle(HttpRequest request, Engine engine, Rooms rooms) async {
  final segments = request.uri.pathSegments;
  final route = segments.isEmpty ? '' : segments.first;
  final name = segments.length == 2 ? segments[1] : null;

  switch ((request.method, route, name)) {
    case ('POST', 'convert', null):
      await _convert(request, engine);
    case ('GET', 'rooms', final String name):
      _send(
        request.response,
        200,
        _format(request, 'format', Format.json),
        rooms[name].doc,
      );
    case ('GET', 'sync', final String name)
        when WebSocketTransformer.isUpgradeRequest(request):
      _sync(await WebSocketTransformer.upgrade(request), rooms[name]);
    default:
      _problem(
        request.response,
        404,
        'notFound',
        'no route for ${request.uri.path}',
      );
  }
}

Future<void> _convert(HttpRequest request, Engine engine) async {
  final from = _format(request, 'from', Format.markdown);
  final to = _format(request, 'to', Format.json);
  final body = await utf8.decodeStream(request);

  if (from == null || to == null) {
    return _problem(
      request.response,
      400,
      'parse',
      'formats are ${Format.values.map((f) => f.name)}',
    );
  }

  try {
    _send(request.response, 200, to, engine.parse(from, body));
  } on CanvistException catch (error) {
    _problem(request.response, 422, error.code.name, error.message);
  }
}

void _sync(WebSocket socket, Room room) {
  room.join(socket);
  socket.listen(
    (data) => room.receive(socket, Uint8List.fromList(data as List<int>)),
    onDone: () => room.leave(socket),
  );
}

Format? _format(HttpRequest request, String parameter, Format fallback) {
  final name = request.uri.queryParameters[parameter];

  return name == null ? fallback : Format.values.asNameMap()[name];
}

void _send(HttpResponse response, int status, Format? format, Doc doc) {
  final to = format ?? Format.json;
  response
    ..statusCode = status
    ..headers.set('content-type', _contentTypes[to]!)
    ..write(doc.export(to));
  unawaited(response.close());
}

void _problem(HttpResponse response, int status, String code, String message) {
  response
    ..statusCode = status
    ..headers.contentType = ContentType.json
    ..write(jsonEncode({'code': code, 'message': message}));
  unawaited(response.close());
}

Future<void> main() async {
  final port = int.tryParse(Platform.environment['PORT'] ?? '') ?? 8787;
  final server = await serve(port: port);
  stdout.writeln('canvist example server on http://localhost:${server.port}');
}
