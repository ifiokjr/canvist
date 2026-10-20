import 'dart:async';
import 'dart:typed_data';

import 'package:canvist_flutter/canvist_flutter.dart';
import 'package:flutter/material.dart';

/// Two collaborators on one document, joined through an in-memory relay
/// that delays every Yjs update a little, as a network would. A real app
/// sends the same updates over a WebSocket (see `doc.receiveSync`).
final class Peers {
  Peers._(this.you, this.peer, this._ownsPeer) {
    you.addListener(_youChanged);
    peer.addListener(_peerChanged);
  }

  /// Share [you]'s document, if it is not shared yet, and let a second peer
  /// join it.
  factory Peers.join(CanvistController you) {
    if (_shared[you.doc] != true) {
      you.doc.share(1);
      _shared[you.doc] = true;
    }

    final doc = you.engine.fromYjs(
      you.doc.encodeState(),
      clientId: _nextClient++,
    );
    return Peers._(you, CanvistController(engine: you.engine, doc: doc), true);
  }

  /// Documents already shared: a document is shared once, however often
  /// peers join it.
  static final _shared = Expando<bool>('shared');
  static var _nextClient = 2;

  /// How long an update takes to arrive.
  static const latency = Duration(milliseconds: 120);

  /// This device's editor.
  final CanvistController you;

  /// The collaborator's editor.
  final CanvistController peer;

  final bool _ownsPeer;
  final _inFlight = <Timer>{};

  void _youChanged() => _relay(you, peer);

  void _peerChanged() => _relay(peer, you);

  void _relay(CanvistController from, CanvistController to) {
    final updates = from.doc.takeUpdates();

    if (updates.isEmpty) {
      return;
    }

    late final Timer timer;
    timer = Timer(latency, () {
      _inFlight.remove(timer);
      _deliver(to, updates);
    });
    _inFlight.add(timer);
  }

  void _deliver(CanvistController to, List<Uint8List> updates) {
    for (final update in updates) {
      to.applyUpdate(update);
    }
  }

  /// Stop relaying and let the peer go, with any updates still in flight
  /// to it.
  void dispose() {
    you.removeListener(_youChanged);
    peer.removeListener(_peerChanged);

    for (final timer in _inFlight) {
      timer.cancel();
    }

    _inFlight.clear();

    if (_ownsPeer) {
      peer
        ..dispose()
        ..doc.dispose();
    }
  }
}

/// Both peers' editors, side by side on wide screens and stacked on narrow
/// ones.
class PeersView extends StatelessWidget {
  const PeersView({
    super.key,
    required this.peers,
    required this.you,
    required this.peer,
  });

  final Peers peers;
  final Color you;
  final Color peer;

  @override
  Widget build(BuildContext context) {
    final editors = [
      _Pane(
        key: const Key('you'),
        label: 'You',
        color: you,
        controller: peers.you,
        autofocus: true,
      ),
      _Pane(
        key: const Key('peer'),
        label: 'Peer',
        color: peer,
        controller: peers.peer,
      ),
    ];

    return LayoutBuilder(
      builder: (context, constraints) {
        final wide = constraints.maxWidth > 720;
        final children = [
          for (final editor in editors) Expanded(child: editor),
        ];

        return Padding(
          padding: const EdgeInsets.all(12),
          child: wide
              ? Row(spacing: 12, children: children)
              : Column(spacing: 12, children: children),
        );
      },
    );
  }
}

class _Pane extends StatelessWidget {
  const _Pane({
    super.key,
    required this.label,
    required this.color,
    required this.controller,
    this.autofocus = false,
  });

  final String label;
  final Color color;
  final CanvistController controller;
  final bool autofocus;

  @override
  Widget build(BuildContext context) {
    return DecoratedBox(
      decoration: BoxDecoration(
        border: Border.all(color: color, width: 2),
        borderRadius: BorderRadius.circular(12),
      ),
      child: ClipRRect(
        borderRadius: BorderRadius.circular(10),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Container(
              width: double.infinity,
              color: color,
              padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 4),
              child: Text(
                label,
                style: TextStyle(
                  color:
                      ThemeData.estimateBrightnessForColor(color) ==
                          Brightness.dark
                      ? Colors.white
                      : Colors.black,
                  fontWeight: FontWeight.w600,
                ),
              ),
            ),
            Expanded(
              child: CanvistEditor(
                controller: controller,
                autofocus: autofocus,
                semanticsLabel: '$label\'s document',
              ),
            ),
          ],
        ),
      ),
    );
  }
}
