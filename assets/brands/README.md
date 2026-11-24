# Third-party marks

## Agent Client Protocol Registry

[ACP's brand guidelines](https://agentclientprotocol.com/brand) supply the
original logomark and permit its use in pure black or pure white. Butler
includes only the unmodified pure-black variant for the labeled ACP Registry
row on light Welcome. It is not Butler branding and does not indicate
affiliation, endorsement, or a working ACP Registry integration.

- Official [brand archive](https://cdn.agentclientprotocol.com/acp-brand.zip),
  SHA-256 `da3087de2628df2aaa2baa443fbeab7983e18a38941199cf6b87df32b6846da8`.
- Unmodified archive member
  `acp-brand/acp-logomark/acp_mark-black.svg`, vendored as
  [acp-mark-black.svg](acp-mark-black.svg), SHA-256
  `d75028b33a5eaf8edd686c2fa70adee10ddd0c31dab614d2f2bd2fe00feb87f7`.
- Its path data and `0 0 576 220` view box exactly match the
  [pinned T3 Code Welcome icon](https://github.com/pingdotgg/t3code/blob/59651c1761f6c63e9b6056839067066968e6a406/apps/web/src/components/Icons.tsx#L468-L477).
  The mark remains centered at its natural aspect ratio in T3's 16px slot;
  no geometry is redrawn or recolored.
- For an offline verification with FontTools 4.61.1, run
  `python3 scripts/export-acp-registry-mark.py --check` from the Butler root.
  To additionally compare the pinned T3 source, add
  `--t3-icons-source /path/to/apps/web/src/components/Icons.tsx`.
  The native Canvas path is generated deterministically from the official SVG;
  runtime rendering does not load an SVG or make a network call.

This approval is scoped to the ACP asset for the ACP-labeled row. It does not
clear other agent, vendor, or operating-system marks.
