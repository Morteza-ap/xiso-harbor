# Experimental Games on Demand output

The output-format selector offers **Extracted files** and **Games on Demand
(Experimental)**. The existing extraction engine remains the folder backend.
The GOD backend is local and offline, uses the same bounded XDVDFS scan, and reads
only a bounded root `default.xex` header. It reuses vendored upstream GOD format
modules, not upstream's CLI or ISO parser.

## Initial limits

Only Xbox 360 images reporting disc 1 of 1 are accepted. Images with root Content
entries, missing/invalid execution metadata, or original Xbox executables are
rejected with guidance to use Extracted files mode. Metadata and directory heuristics
cannot identify every unusual installer; successful conversion does not guarantee
console compatibility. Multi-disc merging, DLC installation, artwork extraction,
FTP, drive preparation and console configuration are not implemented.

GOD conversion preserves the entire detected game partition, including padding;
there is no aggressive trimming or ISO rebuild. Names initially come from the ISO
filename (limited to 100 UTF-16 units). No custom dashboard artwork is read or changed.

## Architecture and safeguards

`main.rs` dispatches the selected OutputFormat to `engine::extract` or
`god::convert`. The queue tracks the output format; switching formats retries
images instead of reusing completions from the other format. Both backends use
app-managed UUID partial folders, recovery records and exclusive final rename.
GOD validates metadata before creating output, estimates space including format
overhead, writes sequentially with bounded memory and cancellable reads, syncs and
reads back each part, verifies data/subtable/master hashes and the part hash chain,
and checks source length/modification time before commit. Failed or cancelled work
remains partial. Existing destination files are never merged or replaced.

The format subset is pinned and attributed in THIRD_PARTY_NOTICES.md. It can be
updated independently of the extraction engine. No runtime shell helper or network
permission is added to the WebView.

## One-game console test

1. Keep your installed working app. Build with `npm run package -- --preview`
   to create **Xiso Harbor GOD Preview.app**. Its separate application identifier
   keeps recovery records and instance locking separate from the installed app.
2. Choose one ordinary single-disc Xbox 360 backup and a new local Mac destination.
3. Select Games on Demand and create the package. The result contains
   `Content/0000000000000000/<TitleID>/00007000/<PackageID>` and its sibling
   `<PackageID>.data/` directory. Keep the pair together.
4. Back up important configuration and saves. Inspect the existing USB Content
   layout. Copy only the new title's files using Finder if normally mounted, or
   Aurora FTP. Decline conflicting replacements. Never replace the whole Content
   directory, reformat, repartition, or modify exploit/profile files.
5. Activate ABadAvatar and XeUnshackle normally. Return to Microsoft's dashboard
   within the same session, without rebooting. DashLaunch documents RB as the NXE
   override; check the installed configuration if exits redirect to Aurora.
6. Separately check My Games visibility, launch, gameplay, saving and loading.
   A package appearing in the library does not prove it can launch. After reboot,
   the temporary exploit and patches must be activated again.
7. If it fails, remove only the new package and data folder. Existing extracted
   games remain available through Aurora.

Native synthetic tests validate content and hashes, cancellation, collisions,
malformed metadata and corruption. They do not establish compatibility with an
actual game or console. This feature remains experimental until tested on hardware.
