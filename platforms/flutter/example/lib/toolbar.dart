import 'package:canvist_flutter/canvist_flutter.dart';
import 'package:flutter/material.dart';

import 'keys.dart';

/// Formatting buttons for a [CanvistController], lit up by the typed
/// queries of the marks and blocks at the selection.
class EditorToolbar extends StatelessWidget {
  const EditorToolbar({super.key, required this.controller, required this.kit});

  final CanvistController controller;
  final StarterKitHandles kit;

  @override
  Widget build(BuildContext context) {
    return ListenableBuilder(
      listenable: controller,
      builder: (context, _) {
        final level = controller.query(kit.heading.headingLevel());

        return SingleChildScrollView(
          scrollDirection: Axis.horizontal,
          padding: const EdgeInsets.symmetric(horizontal: 8, vertical: 4),
          child: Row(
            children: [
              _button('undo', Icons.undo, 'Undo', controller.undo),
              _button('redo', Icons.redo, 'Redo', controller.redo),
              const _Gap(),
              _toggle(
                'heading1',
                Icons.title,
                'Heading',
                level == 1,
                kit.heading.toggleHeading(level: 1),
              ),
              _toggle(
                'heading2',
                Icons.text_fields,
                'Subheading',
                level == 2,
                kit.heading.toggleHeading(level: 2),
              ),
              const _Gap(),
              _mark(
                'bold',
                Icons.format_bold,
                'Bold',
                kit.bold.isBold(),
                kit.bold.toggleBold(),
              ),
              _mark(
                'italic',
                Icons.format_italic,
                'Italic',
                kit.italic.isItalic(),
                kit.italic.toggleItalic(),
              ),
              _mark(
                'underline',
                Icons.format_underline,
                'Underline',
                kit.underline.isUnderline(),
                kit.underline.toggleUnderline(),
              ),
              _mark(
                'strike',
                Icons.format_strikethrough,
                'Strikethrough',
                kit.strike.isStrike(),
                kit.strike.toggleStrike(),
              ),
              _mark(
                'code',
                Icons.code,
                'Code',
                kit.code.isCode(),
                kit.code.toggleCode(),
              ),
              const _Gap(),
              _mark(
                'bulletList',
                Icons.format_list_bulleted,
                'Bullet list',
                kit.bulletList.isBulletList(),
                kit.bulletList.toggleBulletList(),
              ),
              _mark(
                'orderedList',
                Icons.format_list_numbered,
                'Numbered list',
                kit.orderedList.isOrderedList(),
                kit.orderedList.toggleOrderedList(),
              ),
              _mark(
                'taskList',
                Icons.checklist,
                'Task list',
                kit.taskList.isTaskList(),
                kit.taskList.toggleTaskList(),
              ),
              _mark(
                'blockquote',
                Icons.format_quote,
                'Quote',
                kit.blockquote.isBlockquote(),
                kit.blockquote.toggleBlockquote(),
              ),
              _mark(
                'codeBlock',
                Icons.data_array,
                'Code block',
                kit.codeBlock.isCodeBlock(),
                kit.codeBlock.toggleCodeBlock(),
              ),
            ],
          ),
        );
      },
    );
  }

  Widget _button(
    String key,
    IconData icon,
    String tooltip,
    bool Function() onPressed,
  ) => IconButton(
    key: Keys.toolbar(key),
    tooltip: tooltip,
    icon: Icon(icon),
    onPressed: onPressed,
  );

  Widget _mark(
    String key,
    IconData icon,
    String tooltip,
    Query<bool> active,
    Command command,
  ) => _toggle(key, icon, tooltip, controller.query(active), command);

  Widget _toggle(
    String key,
    IconData icon,
    String tooltip,
    bool selected,
    Command command,
  ) => IconButton(
    key: Keys.toolbar(key),
    tooltip: tooltip,
    isSelected: selected,
    icon: Icon(icon),
    onPressed: controller.can(command) ? () => controller.run(command) : null,
  );
}

class _Gap extends StatelessWidget {
  const _Gap();

  @override
  Widget build(BuildContext context) =>
      const SizedBox(height: 24, child: VerticalDivider(width: 16));
}
