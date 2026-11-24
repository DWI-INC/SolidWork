# <img src="assets/solidwork-icon.png" width="64" height="64" align="absmiddle" alt=""> SolidWork

SolidWork is a parametric CAD modeller for macOS, Windows and Linux. Build a part from dimensioned sketches, fit it into an assembly, and generate drawings that update when the model changes. The German-language interface follows the PTC Creo Parametric 10 workflow.

The app is written in Rust and includes its own geometry kernel, constraint solvers and STEP reader. The desktop app also includes structural, thermal and flow simulation, a printed circuit board editor, an FDM slicer, CNC milling and an MCP server for editing parts from an AI assistant.

[Download](#download) · [Build from source](#build--run) · [Feature reference](#features) · [Screenshot gallery](#screenshot-gallery) · [MCP setup](#ai-assistants-mcp)

![SolidWork workspace showing the feature tree and a bracket with a central boss and four mounting holes](docs/screens/bracket.png)

## New in 1.3.0

**Circuit boards and enclosures.** Open *Werkzeuge ▸ Manufacture ▸ Leiterplatte* to draw a schematic, transfer its connections to a board and route the copper. You can use a closed part sketch as the board outline. Choose *3D-Modell* to bring the board, copper and components into a mechanical assembly and check where connectors meet the enclosure.

| Routed LED blinker board | Component models in a SolidWork assembly |
| --- | --- |
| ![LED blinker board with routed copper, pads, vias and a ground pour](docs/screens/pcb_routed.png) | ![Populated sample PCB in the assembly workspace with component geometry and connectors](docs/screens/pcb_showcase_3d_close.png) |

**Idea pages and connector cutouts.** Choose *Datei ▸ Neu ▸ Idee* to sketch a concept, set the pen colour and width, or place a PNG or JPEG reference on the page. SolidWork saves the strokes and images together in a `.prti` file as you work. When you model an enclosure, the sketcher's *Ports* palette provides profiles for USB-C, USB-A, HDMI, Ethernet and other connector openings.

| Idea page with a flange study | Connector profiles in the sketcher |
| --- | --- |
| ![Idea page combining a flange reference image with a six-hole bolt-circle sketch](docs/screens/idea_flange.png) | ![Ports palette open in the sketcher with the USB-A opening selected](docs/screens/ports.png) |

**Calculations in dimension fields.** Enter `(10+2)/3` in an extrusion depth and press Enter to set it to `4`. Enter `HEIGHT-7` to keep a formula that follows the model parameter. Face triangulation also handles parts with many holes more reliably and removes stray edges between openings.

<details>
<summary>PCB design, checks and export formats</summary>

- Design schematics with hierarchical sheets, buses and multi-unit parts. Run the electrical rules check before transferring the netlist to the board.
- Place footprints on either side, route manually with 45° bends or use the two- or four-layer autorouter. The layout editor also supports differential pairs, length tuning and copper zones with thermal reliefs.
- Check clearance, track width, drill spacing, unrouted nets and agreement with the schematic. Inspect the reported violations in the board editor.
- Read and write KiCad projects, schematics and boards. Load `.kicad_sym` libraries and `.pretty` footprints, edit symbols and generate footprints from package dimensions. The 3D view uses KiCad STEP component models when available and can generate package geometry.
- Export Gerber layers, Excellon drill and slot files, pick-and-place coordinates, IPC-2581 and a CSV bill of materials.

</details>

## Interface

The UI follows Creo Parametric 10 (German). Command names below are given in English, with
the German label where it helps to find the command.

- **Startseite** (home screen) before a model is open: common folders, recent files, New and Open
- Title bar with the **Quick Access Toolbar** (New, Open, Save, Undo, Redo, Regenerate, windows, close) and the "BRACKET (Aktiv)" title
- Green **Datei** tab with Creo's file menu, recent files and direct export; ribbon tabs **Modell, Analyse, Live-Simulation, Anmerkungen erstellen, Werkzeuge, Ansicht, Flexible Modellierung, Anwendungen** and **HTL**, with Creo's groups, large/small buttons, group overflow menus and vector icons
- **Contextual feature tabs** (Profil/Extrude, Drehen/Revolve, Bohrung/Hole, Rundung/Round, …) with Placement / Options / Properties slide-down panels and Preview / OK / Cancel; **dialog boxes** for datum plane, axis, point, coordinate system and sketch setup
- **Skizze** tab with Creo's Setup, Einfügen, Operationen, Bezug, Skizze, Editieren, Bedingung definieren, Bemaßung, Prüfen and Schließen groups; sketching happens **in the 3D graphics window** on the sketch plane
- **Weak dimensions**: a sketch is always fully dimensioned automatically (light blue); a strong dimension or constraint replaces the matching weak one, and modifying a weak dimension makes it strong
- **Navigator** with the model tree (default datums, expandable sections, failed/suppressed markers, draggable insert marker, columns, filter) and a folder browser
- **Graphics window**: Creo's graphics toolbar (customisable by right-click), two-sided datum planes with tags, dash-dot axes, spin centre, green selection / blue preselection, the three-row **mini toolbar** next to a selection, and **double-click Edit Dimensions** with in-place value editing
- **Status bar**: message area, regeneration status, placement status, selection count and selection filter
- The **Neu** dialog for parts, assemblies, drawings and Idea pages, **Datei öffnen** dialog with preview and file operations, and the **Create Component** dialog
- Drop-down menus are as wide as their text and slightly rounded
- Flexible Modellierung and some commands on Anwendungen and HTL are shown for layout parity but greyed out (see [Scope](#scope-compared-with-creo))

## Download

When published, installers are available from the [releases page](https://github.com/DWI-INC/SolidWork/releases):

| System | File | Install |
|---|---|---|
| **macOS**, Apple Silicon | `SolidWork-<version>-macos-arm64.dmg` | Open it and drag `SolidWork.app` to Applications |
| **macOS**, Intel | `SolidWork-<version>-macos-x64.dmg` | Open it and drag `SolidWork.app` to Applications |
| **Windows**, x64 | `SolidWork-<version>-windows-x64-setup.exe` | Run the installer (Start menu, desktop icon, file types, uninstaller) |
| **Windows**, ARM64 | `SolidWork-<version>-windows-arm64-setup.exe` | Run the installer |
| **Windows**, 32-bit | `SolidWork-<version>-windows-x86-setup.exe` | Run the installer |
| **Ubuntu / Debian** | `solidwork_<version>_amd64.deb`, `solidwork_<version>_arm64.deb` | `sudo apt install ./solidwork_<version>_amd64.deb` |
| **Fedora / openSUSE / RHEL** | `solidwork-<version>-1.x86_64.rpm`, `solidwork-<version>-1.aarch64.rpm` | `sudo dnf install ./solidwork-<version>-1.x86_64.rpm` |
| **Linux** | `SolidWork-<version>-x86_64.AppImage`, `SolidWork-<version>-aarch64.AppImage` | `chmod +x` the file and start it |

The `.tar.gz` (Linux, macOS) and `.zip` (Windows) files hold the bare programs `solidwork` and
`solidwork-mcp` without an installer; on Windows `SolidWork-<version>-windows-<arch>.exe` is the app as a single file.

- **macOS**: the app is not notarised; on first start choose **Open Anyway** in System Settings > Privacy & Security.
- **Windows**: the programs are not signed; SmartScreen asks once (**More info** > **Run anyway**).
- **Linux**: Ubuntu 22.04 or newer, or a distribution of the same age. The AppImage uses the system's graphics libraries.

SolidWork is developed on macOS. The Linux builds are compiled, tested and started once on Ubuntu
for every release; the Windows builds are compiled on Windows, where the x64 build runs the test suite.

## Build & run

Requires a current stable Rust toolchain, on macOS 11 or later, on Linux (X11 or Wayland) or on Windows 10 or later.

```bash
cargo run --release
```

Build a double-clickable `SolidWork.app` (with icon and document types) in the project root:

```bash
./scripts/bundle.sh
```

Copy `SolidWork.app` to `/Applications` to install it. The bundle is not notarised; after copying,
sign it ad hoc so macOS starts it:

```bash
codesign --force --deep -s - /Applications/SolidWork.app
```

On Linux, `scripts/package-linux.sh` packs the release build into a `.deb`, an `.rpm`, an
AppImage and a `.tar.gz` in `dist/`. The release workflow in `.github/workflows/release.yml`
builds the files for x86-64 and ARM64 on all three systems, plus 32-bit x86 on Windows.

On first launch, SolidWork imports settings and recent files from Freo if available. It reuses
an existing `freo-work` folder when there is no `solidwork-work` folder yet.

Open a model directly with `solidwork part.pfprt` / `solidwork asm0001.pfasm`, from Finder, or by
dragging a file onto the window (a part dropped onto an open assembly is assembled).

## AI assistants (MCP)

SolidWork ships an [MCP](https://modelcontextprotocol.io) server, so Claude, ChatGPT and other
MCP clients can build and edit parts. It runs headless over stdio (JSON-RPC 2.0) on the
same model, regeneration and export code as the app, and keeps one part in memory per session.

```bash
cargo build --release --bin solidwork-mcp      # → target/release/solidwork-mcp
```

The app binary serves the same thing with `solidwork --mcp` (in the bundle:
`SolidWork.app/Contents/MacOS/solidwork --mcp`). Relative file paths in tool calls resolve against
`SOLIDWORK_WORK_DIR`, or the server's working directory if that is not set. Pointing it at
`~/Documents/solidwork-work`, the default work folder on a new installation, makes parts saved by an assistant show
up in SolidWork's Startseite. Existing installations can keep using `~/Documents/freo-work`
and the `FREO_WORK_DIR` environment variable. With `SolidWork.app` installed, `command` can also be
`/Applications/SolidWork.app/Contents/MacOS/solidwork` with `args: ["--mcp"]`.

**Tools:** `new_part`, `open_model` (`.pfprt`, or STEP/STL/OBJ as an import), `save_model`,
`get_model_info`, `list_features`, `get_feature`, `list_geometry` (faces, edges, datums),
`add_datum_plane`, `add_sketch`, `add_extrude`, `add_revolve`, `add_hole`, `add_round`,
`add_chamfer`, `add_shell`, `add_pattern`, `add_mirror`, `add_feature_json`, `edit_feature`,
`suppress_features`, `resume_features`, `delete_features`, `set_parameters`, `set_relations`,
`set_model_properties`, `regenerate`, `undo`, `mass_properties`, `export_model`
(STL, OBJ, STEP, 3MF, PLY, VRML, glTF), `import_geometry` and `render_png` (returns a shaded
picture of the part). Resources: `solidwork://model/current` (the part as JSON) and
`solidwork://model/features` (the model tree).

Every change regenerates the part and returns its volume, bounding box and errors. A change
that makes a feature fail is rolled back and comes back as a tool error. Planes are named
like in the app (`OBEN`/`TOP`, `VORNE`/`FRONT`, `RECHTS`/`RIGHT`, `DTM1`) or picked as faces
(`"+Y"` is the outermost face pointing up). Edges for rounds and chamfers are indices from
`list_geometry` or a filter such as `{"direction": "Y"}`. Every tool carries MCP annotations,
so clients can run read-only tools without asking. Five are marked destructive because they
can discard unsaved work or overwrite files: `new_part`, `open_model`, `save_model`,
`export_model` and `delete_features`.

**Claude Desktop:** add the server to
`~/Library/Application Support/Claude/claude_desktop_config.json` and restart Claude:

```json
{
  "mcpServers": {
    "solidwork": {
      "command": "/path/to/SolidWork/target/release/solidwork-mcp",
      "env": { "SOLIDWORK_WORK_DIR": "/Users/you/Documents/solidwork-work" }
    }
  }
}
```

**Claude Code:**

```bash
claude mcp add solidwork --scope user -e SOLIDWORK_WORK_DIR=$HOME/Documents/solidwork-work -- /path/to/SolidWork/target/release/solidwork-mcp
```

Then ask, for example: *"Build a 100 × 60 × 10 mm aluminium plate with a 10 mm through hole
20 mm right of centre and 5 mm corner rounds, show it, and export plate.stl."*

**ChatGPT and other clients:** clients that start local stdio servers (OpenAI Codex CLI,
Cursor, VS Code, Windsurf, Zed …) use the same command. In Codex, for example, add this to
`~/.codex/config.toml`:

```toml
[mcp_servers.solidwork]
command = "/path/to/SolidWork/target/release/solidwork-mcp"
```

ChatGPT itself (connectors in developer mode) only talks to remote MCP servers over HTTPS. To
use SolidWork there, put a stdio-to-HTTP bridge in front of `solidwork-mcp` (for example
`npx -y supergateway --stdio /path/to/solidwork-mcp --port 8000`) and expose it with a tunnel.
That makes the server, and the files it can read and write, reachable from the internet, so
only do it on a machine and work directory you are fine sharing.

**OpenClaw:** register the server with OpenClaw's own MCP manager. While the Gateway is
running, the CLI needs operator access to it; otherwise quit OpenClaw first so the command
can change the config offline.

```bash
openclaw mcp add solidwork --command /Applications/SolidWork.app/Contents/MacOS/solidwork --arg=--mcp \
  --env SOLIDWORK_WORK_DIR=$HOME/Documents/solidwork-work --timeout 120
```

[`integrations/openclaw/solidwork-cad/SKILL.md`](integrations/openclaw/solidwork-cad/SKILL.md) is an
OpenClaw skill that teaches an agent the SolidWork workflow: planning, plane coordinates, tool
recipes, checking volumes, looking at renders, and sending back renders and export paths.
Copy the `solidwork-cad` folder into an agent's `<workspace>/skills/` (or `~/.openclaw/skills/`
for all agents). It only shows up once `mcp.servers.solidwork` is configured. Check it with
`openclaw skills info solidwork-cad --agent <id>`.

## New in 1.2.0

- **2D drawing to 3D part**: open a **DXF** (or **DWG** with LibreDWG or the ODA File Converter installed) and SolidWork builds the part from its views. Front, top and side views are found by their alignment (the isometric view, frame, title block, dimensions and centre lines are skipped), European or American projection from where the top view lies, the scale from the dimensions or the title block. The part is made of editable features: the top view's outline extruded, everything outside the front and side outlines cut away, holes and pockets cut over the depth their hidden lines show, round bosses shaped. Circles and arcs stay true circles and arcs with exact sizes; a single view becomes a plate (thickness from a "t=5" note). A dialog shows what was found and rebuilds with another projection, scale or thickness
- **3D printing (Werkzeuge ▸ Manufacture ▸ Slicer)**: printer, filament and print profile, orient and arrange on the bed, supports, brim and skirt, slicing with a layer preview, G-code export
- **CNC milling (Werkzeuge ▸ Manufacture ▸ CNC)**: machine, material, stock and zero point; facing, drilling, roughing, contour and 3D finishing; a simulation that removes the material move by move and checks every move for crashes, gouges, holder and table collisions; NC code for Haas/Fanuc-style controls and GRBL. Holes without a matching drill are bored with a helix, curves are cut as G2/G3 arcs, the cutter is clamped as long as tall walls need, and 3D finishing only runs over sloped and curved faces
- **Round, chamfer and hole dashboards like Creo 10**: round sets with Kreisförmig / Konisch / C2 / D1×D2 sections and conic parameter, Vollrundung, Durch Kurve, Sehne, Flächen verlängern, rolling ball or normal to spine, spherical corner transitions, pieces and surface rounds; chamfers D×D, D1×D2, angle×D, 45×D, O×O and O1×O2 with offset-surface or tangent distances, value or reference, corner-chamfer transitions; holes with flat, drill-point or sketched profiles, Variabel / Symmetrisch / Bis zur nächsten / Durch alle / Bis zur ausgewählten, linear, radial, diameter, coaxial and on-point placement, orientation, hole note and lightweight holes; Pause, preview modes and verification in every dashboard
- **Rounds look right**: smooth shading across rounds, no lines where a round runs tangentially into its faces, no stray chords, and concave rounds no longer stick out past the part
- **Threads**: cosmetic threads show their turns on the cylinder; tapped standard holes show them too
- **Sketcher**: circles drawn one after another snap to the first one's radius

## New in 1.1.0

Small things that got in the way, found by using SolidWork like a student would:

- **Safer**: deleting a feature asks first and lists the child features that go with it (including sketches on its faces and rounds on its edges, which used to be left behind and fail); Abbrechen or Esc on a sketch or feature with work in it asks before throwing it away; closing the window asks **Speichern / Nicht speichern / Abbrechen**
- **Typing values**: dimension boxes, renaming in the tree, the Neu dialog and Save As open with the text selected, so typing replaces it; clicking away keeps a changed value; Enter and Esc work in the Neu dialog
- **Undo**: says why it cannot run while a feature is open; undoing back to the saved part no longer marks it unsaved; "fehlgeschlagen" messages disappear once the feature works again
- **Dialogs**: all questions have German buttons; Masseneigenschaften, Modellinfo, Messen and Steuerung use the Creo dialog look; density in kg/m³ with the material shown; the same precision in Messen and the status bar
- **Files**: the Open dialog list no longer jumps when a file is selected, and shows local times; recent files with the same name show their folder; the background colour is remembered
- The view zooms to the first solid of a new part; the model tree columns are never cut off

## New in 1.0.0

- **Windows** builds (x86, x64 and ARM64) and a **macOS build for Intel** next to Apple Silicon
- **Linux**: `.rpm` packages and AppImages next to `.deb` and `.tar.gz`, all also for ARM64
- New screenshots

## New in 0.3.0

- **Analyse tab**: about 30 analyses that measure the model (short edge, wall thickness, section properties, draft, curvature, shadow, build direction for 3D printing, parameter studies); draft, thickness, curvature, shadow and build direction are also painted on the model with a legend
- **Werkzeuge tab**: Creo-style dialogs for Parameter, Beziehungen, Familientabelle, Referenz-Viewer, Such-Tool, Komponentenschnittstelle, Publiziergeometrie, Farbeffekte-Manager, Bildeditor, Schraffur-Designer, Hilfsanwendungen and the Mitteilungsprotokoll; Räumliches Ziel / ThingMark tab
- **Modell tab**: Creo icons and tooltips, keys X (Profil), S (Skizze), R (Rundung), Zusammenführung/Vererbung, Entfernen (removes faces and closes the gap), Rippenleitkurve
- **Fase / Rundung**: picking one edge of an opening asks whether the whole loop should be chamfered or rounded; tangent edge chains are treated as one edge
- **Profil**: a clicked face is used as the sketch plane (also for Definieren...), and a direction arrow on the section flips the side
- **Speed**: chamfers and rounds only cut the model near the tool, and only the changed feature is regenerated; adding or editing a feature takes a fraction of the time it did

## Features

For the PCB editor, Idea pages and connector profiles, see [New in 1.3.0](#new-in-130).

**Modelling (Modell tab)**
- Default datums RECHTS / OBEN / VORNE (RIGHT / TOP / FRONT) and the BASIS coordinate system
- Datum plane (offset, through an axis at an angle), datum axis (two planes, world, normal to a plane), datum point, coordinate system, datum curve through points (spline or polyline), sketch
- Extrude: add/remove, blind / symmetric / to next / to selected / through all, second side, flip, taper angle, thicken (thin wall), solid or surface
- Revolve: add/remove, angle, symmetric, flip; the axis is the sketch centerline
- Sweep: sketched trajectory + section, mitred corners; **helical sweep** (springs, threads; pitch, left/right-handed)
- Blend: parallel blend of any number of sections with offsets, and **rotational blend**
- Hole: simple / counterbore / countersink / **standard** (ISO metric coarse M2–M24 and UNC #4-40 – 1/2-13: tapped with tap drill and cosmetic thread, or clearance close/medium/free), blind with drill tip or through all
- **Cosmetic thread** on a cylindrical surface, **rib** from an open sketch
- Round with **multiple sets**, each with its own radius or a **variable radius**, circular, conic, C2 or D1×D2 sections, or **curvature-continuous** ("Apple corner") in five shapes: one-piece G3, three-piece G3 and G2 (lead-in, arc, lead-out), and relief versions with longer lead-ins; full round, through curve, chord, spherical corners; **Auto Round**; chamfer (D×D, D1×D2, angle×D, 45×D, O×O, O1×O2) on straight and curved edges, corner chamfer and corner transitions
- Shell with removed surfaces, draft about a hinge plane (also variable)
- Pattern: direction (1 or 2 directions), axis, **fill** (square / diamond / triangle grid in a sketched region), **curve** (count or spacing along a sketched curve) and **table**; geometry pattern; mirror selected features or all geometry
- **Surfaces (quilts)**: fill, offset, thicken, solidify, merge, trim, intersect, project, extend, split, copy, replace, boundary blend, style and freestyle
- **Bodies**: new body, union, subtract, intersect and split
- Copy / paste of features, user-defined features (UDF), copy geometry, shrinkwrap, family table, regeneration manager
- Automatic axes (A_n) for holes, revolves, cylinders and patterned holes

**Sketcher**
- Lines (chain, tangent to two circles), rectangles (corner, slanted, centre, parallelogram), circles (centre, concentric, 3-point, 3-tangent), arcs (3-point, centre, 3-tangent, concentric, conic), ellipses (centre, axis ends), interpolating splines
- Fillets (circular, elliptical, with or without keeping the corner as construction lines, and the five curvature-continuous "Apple corner" shapes as one sketch entity), chamfers, centerlines (also tangent), points, coordinate systems, **text** along a line with the bundled or any installed font
- **Skizzierer-Palette**: nine tabs of ready-made shapes: polygons (3 to 20 sides), stars, C/I/L/T profiles, slots, ovals, crosses, rounded rectangles, waves, bore half-sections, dovetail, and real-size steel sections (rectangular hollow sections, L angles, U channels). Click a shape, click in the sketch, then scale and rotate it
- **Import** of DXF, IGES and Adobe Illustrator files into the sketch; whole DXF/DWG drawings open as 3D parts built from their views
- Offset, thicken, project model edges, delete segment (trims at intersections), divide, corner, mirror, rotate/resize, replace, cut / copy / paste, construction mode
- Snapping to points, entities, projected model edges and sketch references, optional grid snap; a References dialog for edges, planes, axes and points
- Automatic H/V constraints while drawing; constraints H, V, ⊥, ∥, =, tangent, coincident/point-on, midpoint, symmetric, fix
- Dimensions: length, horizontal/vertical/aligned distance, point–line, radius, diameter, angle, perimeter, baseline (ordinate) and reference dimensions; lockable; inline editing with numbers or parameter expressions; Modify dialog
- Checks: overlapping geometry, intersections, branches, open ends, shaded closed loops, Feature Requirements; info on intersection point, tangency point and entity
- Levenberg–Marquardt solver with live dragging, DOF readout and rejection of redundant or conflicting constraints

**Model tree & editing**
- Edit Dimensions (double-click on the model), Edit Definition, rename, suppress/resume, hide/show, delete with dependent children, reorder (parent/child aware), insert mode (rollback), failed features flagged with the reason
- Undo/redo, regenerate, object-action workflow (preselect a plane/face/edge, then pick the tool) and action-object collectors
- Parameters, relations (`H = W / 3`, functions, comments) and dimensions driven by expressions, sketch dimensions included
- Werkzeuge tab: search features by name, reference viewer (parents and children), model player (replays the feature history), feature and model info, message log

**Viewing & analysis**
- Spin/pan/zoom (mouse, trackpad, pinch), standard and saved orientations, refit, perspective
- Display styles: shading with reflections, shading with edges, shading, hidden line, no hidden, wireframe; transparency, appearance colours
- Datum display toggles, section view through any datum plane with offset
- Pre-selection highlighting, selection filters, context menus
- Measure (area, length, diameter, distance, angle), mass properties (volume, area, mass, centre of gravity, inertia tensor, bounding box) with material densities, thickness and short-edge checks, model report

**Live-Simulation**
- Four study types on the part: **structural** (linear statics), **thermal** (steady heat conduction), **modal** (natural frequencies and mode shapes) and **fluid** (flow around or through the part)
- Constraints and loads picked on faces: fixed, displacement, frictionless, symmetry; force, moment, pressure, gravity, centrifugal, linear acceleration; temperature, convection, heat flow, heat flux; inlet, outlet, wind
- The solver runs beside the UI on a worker thread and restarts when the model or the study changes
- Results on the model: fringe plot with legend (von Mises stress, displacement, safety factor, temperature, heat flux, flow velocity), animated deformation with scale, stream lines, probes, and a simulation report
- Solvers: voxel finite elements with matrix-free conjugate gradients (statics, heat, modes) and a lattice Boltzmann method (D3Q19) for flow, checked against textbook results in `tests/simulation.rs`

**3D annotations (Anmerkungen erstellen)**
- Dimensions, ordinate and reference dimensions, tolerances, notes, surface finish, datum feature symbols and datum targets, geometric tolerances, placed on annotation planes (flat to screen, FRONT, TOP, RIGHT)
- Annotation features group them in the model tree; move, reattach, remove, clean up dimensions
- Combination states with Creo's state tabs below the graphics window, and a model grid

**Assemblies (`.pfasm`)**
- New ▸ Assembly with ASM_RIGHT / ASM_TOP / ASM_FRONT / ASM_DEF_CSYS; **assemble** part files (also by drag-and-drop or from the folder browser), **create** a new part in the assembly, repeat instances
- **Component Placement** tab: Automatic, Coincident, Distance, Angle Offset, Parallel, Normal, Default and Fix constraints between planar surfaces, cylindrical surfaces (their axes), datum planes, axes, points and straight edges; offset, flip, constraint list, **Allow Assumptions**, live status (no constraints / partially constrained (n DOF) / fully constrained / invalid)
- Drag the component being placed to move it (⇧-drag rotates); the free degrees of freedom follow the mouse while the constraints hold
- Placement solver: Levenberg–Marquardt over each component's rigid transform, solved in model-tree order, with rank-based DOF status; references are stored in part coordinates and follow part edits
- Assembly model tree with placement status markers, the constraints and each component's features; Edit Definition, Open, Hide, Suppress, Delete, Assemble Another Instance
- **Open a component** to edit its features with the rest of the assembly drawn as transparent context, then return to the assembly; saving writes modified parts back to their files
- **Exploded view** with adjustable distance, **Bill of Materials** (quantities, volumes, masses, CSV export), **Global Interference**, assembly **mass properties** from each part's density, per-part colours

**Drawings (`.pfdrw`)**
- New ▸ Drawing with Creo's dialog: default model (the part or assembly in session, or browse), template with front / top / right projection views and a pictorial view scaled to fit, or empty with format; ISO A4–A0 and ANSI A–E sheets, landscape or portrait, third- or first-angle projection
- Drawing format: border with zone markers and a title block (company, title, drawing number, scale, size, sheet *n* of *m*, projection symbol, drawn by, date, material) edited in Drawing Properties
- **Layout** tab: multiple sheets, general views (saved orientations), projection views aligned to their parent, detail views (circular boundary, own scale), 2D cross-sections through a datum plane (hatched cut faces, cutting-plane line with arrows and letters), per-view scale and display style, axes and centre marks, exploded assembly views, Lock View Movement, Open Model (edit in 3D, then return to the drawing)
- Views are generated by hidden-line removal of the model (feature edges and silhouettes of curved faces, depth-tested against the facets) and follow every model change
- **Annotate** tab: Show Model Annotations places the part's sketch dimensions, depths, pattern spacings and hole callouts in the view where they show true length; modifying a shown dimension drives the model. Created (driven) dimensions snap to vertices, edges and circles and re-attach after model edits; `@D` text overrides; notes; Cleanup Dimensions; decimals and text height
- **Dimension tolerances** in a contextual Dimension tab: nominal, limits, plus-minus, symmetric, basic, reference and **ISO 286 fits** (H, G, F, JS holes; h, g, f, js, k, n, p shafts; IT5–IT11 up to 500 mm), tolerance precision, and the general tolerance (e.g. ISO 2768-m) in the title block
- **GD&T**: datum feature symbols, feature control frames for all 14 characteristics with ⌀ zone, Ⓜ/Ⓛ modifiers and up to three datum references, and ISO 1302 surface texture symbols; each has its own contextual tab, and all follow model changes
- **Table** tab: BOM table (index, part number, quantity, mass) above the title block and balloons with leaders
- Drag views, dimensions, notes, balloons and the table; double-click to edit; undo/redo
- **Publish** tab: vector **PDF** (all sheets), **DXF** (R12, layers per line type) and **SVG**

**Manufacturing (Werkzeuge ▸ Manufacture)**

- In *Slicer*, select the printer, filament and print profile. Step through the sliced layers to inspect walls, infill and supports before exporting G-code.
- In *CNC*, set the machine, stock and work origin. Play back the toolpaths to see the remaining stock and inspect any reported collisions before exporting NC code.

| FDM layer preview | CNC stock simulation |
| --- | --- |
| ![Bracket on the print bed with walls and infill coloured by line type](docs/screens/print_layers.png) | ![Simulated end mill removing stock around the bracket boss](docs/screens/cnc_stock.png) |

**Files**

- Idea pages (`.prti`) store pen strokes and embedded reference images together.
- Native `.pfprt` / `.pfasm` / `.pfdrw` (JSON with the full history; a drawing also embeds its model)
- **Import**: STEP AP203 / AP214 / AP242 (B-rep with analytic and B-spline surfaces, tessellated; assemblies are flattened), STL and OBJ, opened as a part or inserted into a part as an import feature
- **Export**: STEP AP214 (faceted B-rep, one body per component), STL (binary/ASCII), OBJ, PLY, VRML 2.0, glTF 2.0 and 3MF
- Native Creo files (`.prt`, `.asm`, `.drw`, with or without version number) are recognised and listed, but their geometry is in PTC's closed format and cannot be read; SolidWork explains this and points to a STEP export of the same model

## Screenshot gallery

<details>
<summary>Explore modelling, simulation, electronics, assemblies and drawings</summary>

### Modelling

| Sketching in 3D (weak + strong dimensions) | Skizzierer-Palette | Extrude tab with Options panel |
|---|---|---|
| ![Sketching in 3D (weak + strong dimensions)](docs/screens/sketch.png) | ![Skizzierer-Palette](docs/screens/palette.png) | ![Extrude tab with Options panel](docs/screens/extrude.png) |
| **Edit Dimensions on the model** | **File menu** | **Datum Plane dialog** |
| ![Edit Dimensions on the model](docs/screens/editdims.png) | ![File menu](docs/screens/file.png) | ![Datum Plane dialog](docs/screens/dialog.png) |
| **Selection mini toolbar** | **Round sets, variable radius** | **Standard (ISO/UNC) tapped hole** |
| ![Selection mini toolbar](docs/screens/mini.png) | ![Round sets, variable radius](docs/screens/roundsets.png) | ![Standard (ISO/UNC) tapped hole](docs/screens/stdhole.png) |
| **Fill pattern in a sketched region** | **Extrude up to the next surface** | **3D annotations** |
| ![Fill pattern in a sketched region](docs/screens/fillpattern.png) | ![Extrude up to the next surface](docs/screens/tonext.png) | ![3D annotations](docs/screens/annot3d.png) |
| **Analysis tools for the active part** | **Whole-loop question for chamfers** | **Relations and local parameters** |
| ![Analysis tools for the active part](docs/screens/analysis.png) | ![Whole-loop question for chamfers](docs/screens/loop_offer.png) | ![Relations and local parameters](docs/screens/parameters.png) |

### Live-Simulation

| Structural (von Mises stress) | Flow around the part | Thermal |
|---|---|---|
| ![Structural (von Mises stress)](docs/screens/sim_structural.png) | ![Flow around the part](docs/screens/sim_fluid.png) | ![Thermal](docs/screens/sim_thermal.png) |

### Leiterplatte (printed circuit boards)

| Schematic with symbol library | Hierarchical sheet and multi-unit op-amp |
| --- | --- |
| ![Schematic with symbol library](docs/screens/pcb_sch.png) | ![Hierarchical sheet and multi-unit op-amp](docs/screens/pcb_sch_hier_insheet.png) |
| **Footprint editor with QFN generator** | **Design rules check** |
| ![Footprint editor with QFN generator](docs/screens/pcb_brd_fped.png) | ![Design rules check](docs/screens/pcb_drc.png) |

### Assemblies

![Assembly](docs/screens/assembly.png)

| Component Placement tab | Exploded view | Bill of Materials + mass properties | Component opened in context |
|---|---|---|---|
| ![Component Placement tab](docs/screens/placement.png) | ![Exploded view](docs/screens/explode.png) | ![Bill of Materials + mass properties](docs/screens/bom.png) | ![Component opened in context](docs/screens/openpart.png) |

### Drawings

![Part drawing](docs/screens/drawing.png)

| Assembly drawing: BOM table + balloons | Created dimensions | Geometric tolerances | Dimension tab: ISO fit H7 |
|---|---|---|---|
| ![Assembly drawing: BOM table + balloons](docs/screens/drawing_asm.png) | ![Created dimensions](docs/screens/drawing_annotate.png) | ![Geometric tolerances](docs/screens/drawing_gdt.png) | ![Dimension tab: ISO fit H7](docs/screens/drawing_tolerance.png) |

</details>

## Controls

| Action | Input |
|---|---|
| Spin | middle-drag, right-drag, ⌥-drag, ⌥ + two-finger scroll |
| Pan | ⇧ + middle/right-drag, ⇧ + two-finger scroll |
| Zoom | scroll wheel, pinch, ⌃ + middle-drag |
| Select / add | click / ⇧- or ⌘-click |
| Edit dimensions | double-click a surface |
| Edit definition | double-click a tree item, or the mini toolbar |
| OK / cancel feature or placement | Enter / Esc |
| Move / rotate the component being placed | drag / ⇧-drag |
| New / open | ⌘N / ⌘O |
| Undo / redo / regenerate | ⌘Z / ⇧⌘Z / ⌘G |
| Sketcher: line chain / corner rectangle | L / R |
| Sketcher: select all / toggle construction | ⌘⌥A / ⇧G |

## Architecture

```
src/kernel   math, corner blend curves (circular, G2, G3), BSP-tree CSG, ear-clipping triangulation, topology (welding, T-junction
             repair, faces, edge chains), feature solid builders, planar-patch offsetting,
             hidden-line removal
src/sketch   sketch model, profile/loop extraction, constraint solver, text outlines,
             DXF / IGES / Illustrator import, the sketcher palette shapes
src/model    features & references, expression evaluator, regeneration engine, surfaces,
             thread and tolerance tables, STEP import, simulation studies, assemblies
             (placement constraints, solver, merged display, BOM), drawings, 2D drawings to 3D
src/sim      Live-Simulation solvers: voxel mesh, finite elements (statics, heat, modes),
             lattice Boltzmann flow, and the study runner
src/app      controller, OpenGL renderer, camera, picking, sketcher UI, feature tabs,
             ribbon / tree / status bar, dialogs, 3D annotations, Live-Simulation UI,
             assembly and drawing modes, export & mass properties, demo scenes
src/slicer   3D printing: mesh slicing, polygons, infill, supports, path planning, G-code, printers
src/cam      CNC: tools, machines, materials, height-field stock, toolpaths, simulation and
             crash check, NC code
src/pcb      printed circuit boards: symbols, footprints, schematic (netlist, ERC, hierarchy),
             board (ratsnest, zones), DRC, autorouter and interactive routing helpers, Gerber /
             Excellon / IPC-2581 / pick-and-place, KiCad s-expression files, 3D part models
src/mcp.rs   MCP server for AI assistants (stdio JSON-RPC, tools over model/regen/export)
```

References to faces and edges are stored geometrically and re-resolved on every
regeneration, so downstream features follow upstream dimension changes.

## Testing

```bash
cargo test                                   # unit + end-to-end workflow tests
cargo test --test mcp_server                 # MCP server driven over stdin/stdout
cargo test --test simulation                 # solvers against textbook results
cargo test --test pcb                        # schematic, board, DRC, Gerber, KiCad files
FREO_KICAD_DEMOS=<kicad>/demos cargo test --test kicad_real   # KiCad's own demo projects
cargo run --release --bin kernel_demo -- out # regenerate sample parts and render PNGs
cargo run --release --example stepcheck -- <folder>   # import every STEP file in a folder
```

Documentation screenshots come from scripted demo scenes (`src/app/demo.rs`):

```bash
FREO_DEMO=bracket FREO_SHOT=shot.png FREO_EXIT=1 cargo run --release
FREO_DEMO=sim:structural FREO_SHOT=sim.png FREO_EXIT=1 cargo run --release
FREO_DEMO=sketch FREO_DEMO_POPUP=palette:8 FREO_SHOT=palette.png FREO_EXIT=1 cargo run --release
```

## Scope compared with Creo

SolidWork reimplements the core part-modelling, assembly, drawing, annotation and live-simulation
workflow. It does not include Creo's sheet metal, mechanisms, mould and weld applications or
flexible modelling; the Flexible Modellierung tab and some Anwendungen and HTL commands are
shown greyed out. Native Creo files cannot be read (use STEP).

The kernel is polyhedral: curved surfaces are faceted, so STEP export is a faceted B-rep,
imported STEP geometry is a tessellated import feature, and drawing views of curved surfaces
show their facets' silhouettes, rather than an exact NURBS B-rep. Live-Simulation works on a
voxel mesh and is meant for quick design feedback, not for certification.

SolidWork is MIT-licensed (see `Cargo.toml`). "Creo" is a trademark of PTC Inc.; SolidWork is an independent
project and is not affiliated with PTC.
