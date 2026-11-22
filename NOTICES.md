# Notices

## Scope of the MIT licence

The MIT licence in `LICENSE-MIT` applies to this project's original source code. It does not apply to
imported Diablo II: Resurrected content or proprietary dependencies.

## Patched crates

Five crates of the build are official crates.io releases with edits applied. `patches/dependencies.json` pins, for each
of them, the official archive URL, the archive SHA-256, the complete original file map, the complete patched file map and
the SHA-256 of the edit file. `patches/patched_crates.py` rebuilds the patched crates from the archives and the edit
files and checks every file against those pins. No patched source tree is stored in this repository.

| Package | Version | Declared licence | Official archive SHA-256 | Edit file |
| --- | --- | --- | --- | --- |
| iridia | 0.5.0 | GPL-3.0-or-later | `57705f4c0ace24dd5236014c8506f368e9784275cad83596368a40138bed28f8` | `iridia-0.5.0.edits.json` |
| iridia-dxbc | 0.5.0 | GPL-3.0-or-later | `43f5fdb162bf0867c740575ba085deed773bb54f2484fabeac64d97e971235b2` | `iridia-dxbc-0.5.0.edits.json` |
| iridia-ir | 0.5.0 | GPL-3.0-or-later | `637a77a4aa95fd797f6e80ca015e973feec7a7f0d2d6a97d0e10117f170dab14` | `iridia-ir-0.5.0.edits.json` |
| naga | 30.0.1 | MIT OR Apache-2.0 | `a616d2fb8c89516ac2723a581f69d6c18576046bed761bd6b305e5618e6ae130` | `naga-30.0.1.edits.json` |
| primase-naga | 30.0.1 | MIT OR Apache-2.0 | `64db6f77483f8d3fb74c4a7b8925b87e05b8a0a603ff4439500ec6393083d8bd` | `primase-naga-30.0.1.edits.json` |

The licence and notice files shipped in these archives stay unchanged in the patched crates (pinned in
`retained_license_notice_files`). The three iridia archives declare GPL-3.0-or-later in their package metadata and
contain no separate licence file; the GPL text is in `LICENSE` and `licences/standard-licences/GPL-3.0.txt`. naga and
primase-naga keep their MIT and Apache-2.0 notice files. Package metadata is unchanged except where an edit file
changes it.

What the edits add, in short:

- iridia: precise-arithmetic controls for the DXBC lift (bit-exact copies, shifts and raw loads, `round_ni`, `lt`,
  `movc`, `frc`, `max`, `add_sat`, and a tolerance class for `sincos`, `rsq` and `div`, each checked against its
  documented D3D semantics), vertex interface layout fixes, and test-manifest entries for the upstream tests;
- iridia-dxbc: lift rules for the same instructions and for the D3D NaN tests `eq x, x` / `ne x, x`;
- iridia-ir: a varying-interface flag that lets a provably unused generic vertex input be left out of the entry interface;
- naga and primase-naga: correspondence reports for the SPIR-V front and back ends and the compaction pass, an
  arithmetic-control intermediate form with its validation, and the precise-arithmetic controls in the SPIR-V writer.
