# Incular gaps found in Butler

Butler uses [Incular `71feef9`](https://github.com/FabortLabs/incular/tree/71feef92286dfd6f8accd9c17c14f292da746490). This is the app-side log of differences from Flutter, not the list of things Butler still needs to copy from T3 Code. Incular's [own gap log](https://github.com/FabortLabs/incular/blob/71feef92286dfd6f8accd9c17c14f292da746490/INCULAR-GAPS.md) covers other apps and may describe older behavior, so recheck the pinned code before carrying an entry over.

When a gap affects Butler, record the Incular revision, platform or renderer if relevant, minimal reproduction, Flutter behavior, Incular behavior, and the proposed upstream fix. If it needs an Incular fix, log it here rather than adding a workaround to Butler. Continue independent Butler work and verify the framework fix before removing the entry from the open list. This includes behavior, functionality, accessibility, layout, typography, and colors. Mark a difference as source-verified, runtime-verified, or still under investigation. Recheck open entries when updating the Incular pin.

## Verified at the current pin

### IG-001: Raw text input cannot take an explicit focus node or opt into autofocus

**Status:** Source-verified API gap in the low-level widget. Flutter's [`EditableText.focusNode`](https://api.flutter.dev/flutter/widgets/EditableText/focusNode.html) and [`autofocus`](https://api.flutter.dev/flutter/widgets/EditableText/autofocus.html) allow a custom-styled input to take focus when a picker opens, without a prior click. The pinned Incular [`EditableText` descriptor and builders](https://github.com/FabortLabs/incular/blob/71feef92286dfd6f8accd9c17c14f292da746490/crates/incular-widgets/src/tree/descriptors.rs#L989-L1044) expose neither of those direct options. This is not a claim that Incular has no autofocus: its [Material `TextField` has an autofocus builder](https://github.com/FabortLabs/incular/blob/71feef92286dfd6f8accd9c17c14f292da746490/crates/incular-material/src/inputs/text_fields.rs#L497-L505).

**Butler:** The custom [Local folder picker](src/project_picker.rs) uses the raw input. Its [native interaction test](scripts/capture-desktop-startup.sh) clicks the field before typing, so it doesn't prove focus-on-open.

**Repro / acceptance:** Open a palette containing a custom raw text input. Type immediately, without clicking. An explicit focus request on that field should make the first keystroke land there. Also test closing the palette and restoring the previous focus.

### IG-002: Custom `ActionSurface` content bypasses its padding

**Status:** Source-verified behavior on the lower-level surface used by Butler, not a blanket claim about Incular's Material buttons. Flutter's [`ButtonStyle.padding`](https://api.flutter.dev/flutter/material/ButtonStyle/padding.html) pads an arbitrary button child. In Incular's [action-surface lowering](https://github.com/FabortLabs/incular/blob/71feef92286dfd6f8accd9c17c14f292da746490/crates/incular-widgets/src/tree/widget/constructors.rs#L434-L457), the label-only branch wraps the content in padding, but the custom-child branch passes the child through unchanged.

**Butler evidence:** The [folder palette's existing custom action](src/project_picker.rs) defines its own row padding. The framework should apply the surface padding consistently; do not add another app-side workaround.

**Repro / acceptance:** Give a label-only action and a custom-child action the same bounds and 16 px horizontal padding. The child in the second action should be inset without shrinking the hit target. Keep upper-level Material behavior as a separate test.

### IG-003: Selection offsets use different units

**Status:** Source-verified API difference, not a rendering defect. Incular's [`TextRange` and selection clamping](https://github.com/FabortLabs/incular/blob/71feef92286dfd6f8accd9c17c14f292da746490/crates/incular-text/src/editing.rs#L18-L25) use UTF-8 byte offsets. Flutter's [selection offsets](https://api.flutter.dev/flutter/services/TextSelection/baseOffset.html) index a [Dart UTF-16 string](https://api.dart.dev/dart-core/String-class.html). For `a🙂b`, the caret after the emoji is at byte offset 5 in Incular and code-unit offset 3 in Flutter.

**Butler implication:** Convert offsets at a Flutter or UTF-16 protocol boundary; don't copy numeric offsets directly into selection, composing state, or a ported test. Native caret movement itself has not been found to differ here.

### IG-004: Standard modal routes do not contain keyboard focus

**Status:** Source-verified behavior gap. Flutter's in-window [Material `showDialog`](https://api.flutter.dev/flutter/material/showDialog.html) defaults to closed-loop focus traversal. Incular explicitly documents that [modal route focus containment is unimplemented](https://github.com/FabortLabs/incular/blob/71feef92286dfd6f8accd9c17c14f292da746490/crates/incular-navigation/src/presentation.rs#L62-L68); the pointer barrier alone does not provide the traversal policy. Incular's [Material dialog handle](https://github.com/FabortLabs/incular/blob/71feef92286dfd6f8accd9c17c14f292da746490/crates/incular-material/src/feedback/dialog_handles.rs#L83-L117) does not expose a replacement for that policy. This is independent of returning focus to the trigger after a route closes.

**Repro / acceptance:** Place two focusable controls inside an in-window modal and one outside. Repeated Tab from the last control should cycle to the first; repeated Shift+Tab from the first should cycle to the last. The underlying control should never gain focus while the modal is open. Runtime twin check pending.

### IG-005: Stock Material dialog scrim differs in color and theme handling

**Status:** Source-verified visual and theme difference. Flutter's [`showDialog`](https://api.flutter.dev/flutter/material/showDialog.html) uses an explicit scrim color first, then the dialog theme's color, then `Colors.black54` (alpha 138/255). The pinned [Incular Material `DialogHandle`](https://github.com/FabortLabs/incular/blob/71feef92286dfd6f8accd9c17c14f292da746490/crates/incular-material/src/feedback/dialog_handles.rs#L32-L41) defaults to black at alpha 96/255 and uses that stored color unless the caller applies [`barrier_color`](https://github.com/FabortLabs/incular/blob/71feef92286dfd6f8accd9c17c14f292da746490/crates/incular-material/src/feedback/dialog_handles.rs#L67-L99). Butler's custom blurred overlays use their own styles, so their images do not prove this stock-widget difference.

**Repro / acceptance:** Show each framework's stock Material dialog over white with default theme. Sample a backdrop pixel outside the dialog after the animation. Repeat with a dialog theme specifying a different barrier color. Fix the stock default and theme resolution in Incular rather than overriding them in Butler. Runtime twin check pending.

### IG-006: WGPU paints a variable face at its default weight

**Status:** Runtime-verified on Butler's pinned Incular, using the WGPU backend's CPU glyph rasterizer. Flutter's [Skia paragraph builder](https://github.com/flutter/flutter/blob/master/engine/src/flutter/txt/src/skia/paragraph_builder_skia.cc#L173-L195) forwards the variation design position, including the requested weight when no explicit `wght` is supplied. Incular [passes the requested style to Parley](https://github.com/FabortLabs/incular/blob/71feef92286dfd6f8accd9c17c14f292da746490/crates/incular-text/src/engine.rs#L2009-L2023), but the [retained glyph run](https://github.com/FabortLabs/incular/blob/71feef92286dfd6f8accd9c17c14f292da746490/crates/incular-rendering/src/glyphs.rs#L16-L21) does not carry variation coordinates. Both [hinted and unhinted rasterization](https://github.com/FabortLabs/incular/blob/71feef92286dfd6f8accd9c17c14f292da746490/crates/incular-wgpu/src/glyph_rasterizer.rs#L147-L188) use the default location.

**Repro:** Register Butler's bundled Inter Variable face. Shape the same `H` at 48 px with W400, W600 and W900. All three select glyph 38 and the same font bytes, but their advances are 35.671875, 35.789063 and 36.0 px. Rasterizing each into its own fresh glyph atlas, at scale 1 and phase (0,0), produces the same 980-byte coverage mask in all three cases. The static Liberation Sans W400/W700 control selects distinct faces and produces distinct masks. Separate atlases rule out a cache collision as the sole cause. This does not establish a defect for static font faces or other backends.

**Butler effect / acceptance:** Settings uses the original variable Inter face. Its shaped advances respond to weight, but this renderer paints the default W400 outlines. Forward the run variation coordinates through painting and both raster paths, key the glyph and hint caches by the variation instance, and verify that distinct weights produce distinct masks while repeated identical instances reuse their atlas entry. No Butler workaround.

### IG-007: Raw Enter completes a single-line edit with IME preedit still active

**Status:** Runtime-verified for synthetic raw input to the pinned Incular runtime, not for every native input method. Flutter's [`EditableText.onSubmitted` testing contract](https://api.flutter.dev/flutter/widgets/EditableText/onSubmitted.html) distinguishes a raw framework Enter event from a platform-supplied text input action. On Linux, Flutter’s [text input handler](https://github.com/flutter/flutter/blob/master/engine/src/flutter/shell/platform/linux/fl_text_input_handler.cc#L453-L492) gives GTK's input method first refusal before turning an unconsumed Return into a text action. It does not guarantee that every native Enter will be consumed by the IME.

**Repro:** Render a default single-line Incular `EditableText` with text `draft` and click inside it. Dispatch `InputEvent::Ime(ImeEvent::Preedit { text: "候補".into(), selection: None })`; verify the controller reports the active preedit. Without a commit or end-composition event, dispatch `InputEvent::Key(KeyboardEvent::key_down(KeyboardKey::Named(NamedKey::Enter), Code::Enter))`. The field loses focus, preedit becomes `None`, and the committed text stays `draft`. The pinned [key handler](https://github.com/FabortLabs/incular/blob/71feef92286dfd6f8accd9c17c14f292da746490/crates/incular-runtime/src/frame/text_actions.rs#L69-L80) maps raw Enter to the editing action; [single-line completion](https://github.com/FabortLabs/incular/blob/71feef92286dfd6f8accd9c17c14f292da746490/crates/incular-runtime/src/frame.rs#L2322-L2340) then releases focus.

**Butler effect / acceptance:** A composing Enter that reaches the runtime can cancel the provisional name text or trigger a focus-loss commit before the input method finishes. Distinguish raw input-method candidate handling from explicit completion when preedit is active. Verify that a candidate-selection Enter does not blur; the subsequent explicit completion or ordinary Enter with no composition still completes exactly once. A controlled native IME comparison is pending. No Butler interception or workaround.

### IG-008: RichText ignores an inline span's text background

**Status:** Display-list verified on pinned Incular. Flutter's [TextStyle.backgroundColor](https://api.flutter.dev/flutter/painting/TextStyle/backgroundColor.html) specifies paint behind the text, and [TextSpan](https://api.flutter.dev/flutter/painting/TextSpan-class.html) can override the style for only part of a paragraph. Incular's public TextStyle stores the background value, but the pinned [paragraph shaping style translation](https://github.com/FabortLabs/incular/blob/71feef92286dfd6f8accd9c17c14f292da746490/crates/incular-text/src/engine.rs#L2007-L2035) does not forward it.

**Repro:** In a RichText paragraph, use three child TextSpans for “The Café ”, “lantern”, and “ is lit.” Set the middle span's foreground to RGBA (33, 24, 10, 255) and background to (238, 165, 43, 255). Run a retained frame. The middle span has a colored GlyphRun with its own shaped range, but the display list contains no corresponding amber paint behind it. A control inspection after scrolling confirms the colored run is on-screen, so visibility or clipping is not responsible. This observation is for the normal RichText path, not all forms of text painting.

**Butler effect / acceptance:** Transcript search can reveal the message and style the matching foreground, but cannot show T3's contrasting inline background through the standard span API. Render the background only for the styled glyph range, without altering text shaping, selection, or wrapping, including a match crossing a line wrap. No Butler workaround.

## Investigations, not confirmed Incular gaps

- Butler's compact scratch button clipped its label before switching from an outer width constraint to `ButtonStyle::fixed_size` ([native capture before the fix](https://github.com/Ferrb9579/butler/actions/runs/45329928604)). Reproduce the same widget constraints, font and padding in Flutter before blaming framework layout.
- The differences in Butler's T3 screenshot pairs, including window chrome, icons and colors, do not establish Flutter parity bugs. Butler sets much of that styling itself. For a color or rendering report, capture the same widget, font, scale, theme, interaction state and backend where possible.
