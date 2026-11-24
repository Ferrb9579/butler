# Incular gaps found in Butler

Butler uses [Incular `71feef9`](https://github.com/FabortLabs/incular/tree/71feef92286dfd6f8accd9c17c14f292da746490). This is the app-side log of differences from Flutter, not the list of things Butler still needs to copy from T3 Code. Incular's [own gap log](https://github.com/FabortLabs/incular/blob/71feef92286dfd6f8accd9c17c14f292da746490/INCULAR-GAPS.md) covers other apps and may describe older behavior, so recheck the pinned code before carrying an entry over.

When a gap affects Butler, record the Incular revision, platform or renderer if relevant, minimal reproduction, Flutter behavior, Incular behavior, and the workaround. This includes behavior, functionality, accessibility, layout, typography, and colors. Mark a difference as source-verified, runtime-verified, or still under investigation. Recheck open entries when updating the Incular pin.

## Verified at the current pin

### IG-001: Raw text input cannot take an explicit focus node or opt into autofocus

**Status:** Source-verified API gap in the low-level widget. Flutter's [`EditableText.focusNode`](https://api.flutter.dev/flutter/widgets/EditableText/focusNode.html) and [`autofocus`](https://api.flutter.dev/flutter/widgets/EditableText/autofocus.html) allow a custom-styled input to take focus when a picker opens, without a prior click. The pinned Incular [`EditableText` descriptor and builders](https://github.com/FabortLabs/incular/blob/71feef92286dfd6f8accd9c17c14f292da746490/crates/incular-widgets/src/tree/descriptors.rs#L989-L1044) expose neither of those direct options. This is not a claim that Incular has no autofocus: its [Material `TextField` has an autofocus builder](https://github.com/FabortLabs/incular/blob/71feef92286dfd6f8accd9c17c14f292da746490/crates/incular-material/src/inputs/text_fields.rs#L497-L505).

**Butler:** The custom [Local folder picker](src/project_picker.rs) uses the raw input. Its [native interaction test](scripts/capture-desktop-startup.sh) clicks the field before typing, so it doesn't prove focus-on-open.

**Repro / acceptance:** Open a palette containing a custom raw text input. Type immediately, without clicking. An explicit focus request on that field should make the first keystroke land there. Also test closing the palette and restoring the previous focus.

### IG-002: Custom `ActionSurface` content bypasses its padding

**Status:** Source-verified behavior on the lower-level surface used by Butler, not a blanket claim about Incular's Material buttons. Flutter's [`ButtonStyle.padding`](https://api.flutter.dev/flutter/material/ButtonStyle/padding.html) pads an arbitrary button child. In Incular's [action-surface lowering](https://github.com/FabortLabs/incular/blob/71feef92286dfd6f8accd9c17c14f292da746490/crates/incular-widgets/src/tree/widget/constructors.rs#L434-L457), the label-only branch wraps the content in padding, but the custom-child branch passes the child through unchanged.

**Butler workaround:** Put the padding inside the child of the [folder palette's custom action](src/project_picker.rs), preserving a full-row click target and an explicit accessible name.

**Repro / acceptance:** Give a label-only action and a custom-child action the same bounds and 16 px horizontal padding. The child in the second action should be inset without shrinking the hit target. Keep upper-level Material behavior as a separate test.

### IG-003: Selection offsets use different units

**Status:** Source-verified API difference, not a rendering defect. Incular's [`TextRange` and selection clamping](https://github.com/FabortLabs/incular/blob/71feef92286dfd6f8accd9c17c14f292da746490/crates/incular-text/src/editing.rs#L18-L25) use UTF-8 byte offsets. Flutter's [selection offsets](https://api.flutter.dev/flutter/services/TextSelection/baseOffset.html) index a [Dart UTF-16 string](https://api.dart.dev/dart-core/String-class.html). For `a🙂b`, the caret after the emoji is at byte offset 5 in Incular and code-unit offset 3 in Flutter.

**Butler implication:** Convert offsets at a Flutter or UTF-16 protocol boundary; don't copy numeric offsets directly into selection, composing state, or a ported test. Native caret movement itself has not been found to differ here.

## Investigations, not confirmed Incular gaps

- Butler's compact scratch button clipped its label before switching from an outer width constraint to `ButtonStyle::fixed_size` ([native capture before the fix](https://github.com/Ferrb9579/butler/actions/runs/45329928604)). Reproduce the same widget constraints, font and padding in Flutter before blaming framework layout.
- The differences in Butler's T3 screenshot pairs, including window chrome, icons and colors, do not establish Flutter parity bugs. Butler sets much of that styling itself. For a color or rendering report, capture the same widget, font, scale, theme, interaction state and backend where possible.
