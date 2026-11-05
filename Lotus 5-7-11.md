# Lotus 5-7-11

## A Modern Computational Spreadsheet and Structured Human–AI Workspace

**Concept and Technical Architecture Report**

**Status:** Initial architectural proposal\
**Working name:** Lotus 5-7-11\
**Primary implementation candidates:** C or Rust\
**Native storage model:** One uncompressed binary workbook blob\
**Logical document model:** XML-defined structure and object semantics; XML is not required to be the on-disk byte encoding\
**Programming environment:** Spreadsheet formulas and an embedded modern BASIC dialect\
**Primary design objective:** A portable, self-contained computational workspace suitable for ordinary spreadsheet use and precise, interactive collaboration between humans and AI systems.

---

# 1. Executive Summary

Lotus 5-7-11 is a proposed modernization of the computational spreadsheet concept pioneered by Lotus 1-2-3. It retains the spreadsheet as a general-purpose computational environment while extending the underlying document model to support structured instructions, embedded programs, associated documents, explicit references, and machine-readable context.

The project is not intended to recreate Microsoft Excel, reproduce the Microsoft Office suite, or adopt the complexity of modern Office document packaging. Its objective is to recover and modernize the useful ideas behind the programmable spreadsheet.

The central architectural premise is straightforward:

**A spreadsheet can be more than a grid of cells. It can be a portable, executable, inspectable document that represents data, rules, procedures, relationships, and supporting knowledge in one coherent artifact.**

The system would have five principal characteristics:

1. A native, uncompressed binary blob format governed by a purpose-built logical document model, rather than Microsoft's OOXML package format.
2. A spreadsheet calculation engine capable of evaluating formulas, maintaining dependencies, and updating computed state.
3. An embedded BASIC environment for procedural computation, automation, and operations that are awkward to express through formulas alone.
4. A document graph that allows embedded documents, programs, resources, and other structured objects to reference spreadsheet cells and one another.
5. An interface designed for ordinary users while providing AI systems with an explicit, structured representation of the workbook's state and permitted operations.

The project has two complementary purposes. It is an end-user spreadsheet application, and it is a structured medium for human–AI interaction.

These purposes reinforce each other. A human can manipulate the workbook through a familiar grid, while an AI can inspect and modify the same underlying artifact through its formal structure rather than relying exclusively on natural-language descriptions.

The intended result is a compact, portable computational environment whose document format is also a useful interface for machine collaboration.

# 2. Design Philosophy

## 2.1 Recover the computational model, not the historical limitations

Lotus 1-2-3 established a powerful interaction model: a grid of values, formulas that refer to other cells, immediate recalculation, and facilities for automating operations.

That model remains useful because it makes computation visible. Inputs, intermediate results, dependencies, and outputs can coexist in the same working environment.

Lotus 5-7-11 would preserve this general philosophy while replacing historical implementation constraints with modern software architecture.

The intention is not necessarily to reproduce every legacy interface convention or undocumented quirk. It is to investigate the original implementation, identify valuable behavior, and deliberately decide what to preserve.

The project should favor:

- Immediate, understandable interaction.
- Explicit computational relationships.
- A compact runtime.
- A transparent and documented native format.
- Local execution and user control.
- Minimal mandatory dependencies.
- Extensibility without unnecessary architectural complexity.

The historical implementation can serve as a source of behavioral knowledge without dictating the architecture of its replacement.

## 2.2 The workbook is the primary artifact

The workbook is one unified blob, not a filesystem tree and not a ZIP-like package containing many separate files. Its internal structure is addressable, but its physical storage remains one object that can be moved, copied, mapped, or transferred as a unit.

The logical model uses XML-style structure and explicit metadata, but the saved workbook is binary rather than an ASCII/XML text file. The XML representation is a structural model/view used by the application, including for layered historical overlays; it does not require the native blob to be serialized as literal XML text. The format is uncompressed by design.

The logical model can define and relate:

- Worksheets, cells, ranges, and styles.
- Formulas and named expressions.
- BASIC source code and program metadata.
- Embedded documents and structured notes.
- References between documents and spreadsheet objects.
- Validation rules and constraints.
- Calculation settings and workbook state.
- Optional binary resources.
- Provenance, revision information, and other metadata.

The application interprets and executes the logical document model, while the binary blob stores its content and metadata. A reader can inspect the structure through the application's parser or a defined structural view.

The design does not require all content to be loaded into memory at once. The binary layout must support locating and reading named regions directly.

## 2.3 Avoid unnecessary containerization

Modern spreadsheet formats frequently introduce a packaging layer containing numerous interrelated files. That approach can be useful for compatibility and large, complex document ecosystems, but it also creates overhead for simple inspection, generation, modification, and exchange.

Lotus 5-7-11 would not inherit that packaging model merely because contemporary office software uses it.

A purpose-built binary blob can provide the same logical organization without requiring a text encoding or a package of separate files. The workbook itself is the container. A consumer can read a named object's byte range directly without extracting a directory tree or creating temporary files.

A conventional filesystem is an interoperability boundary, not the fundamental storage model. When an external program requires an ordinary file, a cell can be ejected to materialize that file. If the consumer can operate on a byte range or memory buffer, extraction is unnecessary.

# 3. Logical Structure and Binary Serialization

## 3.1 XML-defined structure, binary native storage

Lotus 5-7-11 does not use Microsoft's Office Open XML package format, nor does it need to save the workbook as a literal ASCII/XML document. The native workbook is one uncompressed binary blob. A purpose-built XML-style schema defines the logical object model and relationships; the binary serialization stores that model, its metadata, and its payloads.

This separation allows the application to maintain a structured XML representation for parsing, editing, and historical overlays while storing the actual workbook as one binary object. XML is the vocabulary of the logical model, not a requirement that every stored byte be text.

A possible high-level structure is:

```xml
<lotus-workbook version="1.0" id="workbook-001">

  <metadata>
    <title>Example Workspace</title>
    <calculation-mode>automatic</calculation-mode>
  </metadata>

  <sheets>
    <sheet id="sheet-main" name="Main">
      <cell id="input-a" ref="A1" type="number">10</cell>
      <cell id="input-b" ref="A2" type="number">20</cell>
      <cell id="result" ref="A3" formula="=A1+A2"/>
    </sheet>
  </sheets>

  <programs>
    <program id="program-main" language="basic">
      <!-- Embedded BASIC source -->
    </program>
  </programs>

  <documents>
    <!-- Embedded supporting documents -->
  </documents>

  <resources>
    <!-- Optional binary or external resources -->
  </resources>

</lotus-workbook>
```

This is illustrative syntax, not a finalized schema.

The final design must distinguish document structure from execution semantics. XML defines how information is represented and linked; the formula and BASIC specifications define what the represented instructions actually mean.

## 3.2 Readability and machine generation

The format should be practical for both humans and software to inspect.

The logical model should be inspectable through tooling, and an AI or external program should be able to locate a particular cell, formula, procedure, or payload through the defined index without unpacking many files. Because the native storage is binary, opening the workbook in a plain text editor is not a design requirement. A readable XML export or diagnostic view can still be provided when useful.

The design should also avoid making serialization order carry unintended meaning. Object identity and explicit references should determine relationships.

## 3.3 Stable identities

Cells need both user-facing addresses and, where appropriate, stable identifiers.

A coordinate such as `Main!A3` is convenient for users and formulas. However, moving or inserting cells can change coordinates. An object identifier can preserve the identity of a particular logical object across such changes.

The implementation should define which identities remain stable through editing operations and how references are updated when a cell is deleted, copied, moved, or replaced.

Not every cell necessarily needs a globally persistent identifier. Stable identities are especially useful for objects referenced by external documents, AI operations, procedures, annotations, and audit records.

## 3.4 Named cells are file-like blobs

A cell can be a named blob, not merely a spreadsheet value or a pointer to an attachment. Its metadata carries its literal name, which can be a filename. Its payload is the file's byte sequence, beginning at offset `0x00000000`; the cell is the file in the workbook's unified storage model. A cell can hold arbitrary bytes without the workbook needing to parse the payload's internal format.

Ejecting a cell exports its payload as a standalone file, preserving byte order, embedded zero bytes, and exact length. Conversely, importing a file can create a named cell containing the file's bytes. The same blob can be consumed in place by software that accepts a byte range or memory buffer, without creating a temporary file or touching a conventional filesystem. File extraction is optional interoperability.

The workbook therefore behaves as a single uncompressed container of named file-like objects. It avoids the overhead of managing a directory tree of many small files when software can operate on the workbook blob directly. Efficient access requires an index from cell names to payload regions and lengths; edits must preserve or update those boundaries safely.

A cell's name is its file-facing identity, while its grid coordinates locate it in the raster workspace. The format must define collision handling and scope for names. Metadata must also distinguish the payload from cell display, type, execution role, and other workbook properties.

## 3.5 Spatial addressing and table of contents

The workbook uses a discrete grid with independently expanding X and Y dimensions. A table of contents (TOC) can be placed one cell beyond the maximum used extent on each axis. For example, if the maximum used column is `C` and the maximum used row is `26`, the TOC is at `D27`. If the furthest used cells lie at `C5` and `B26`, the same rule yields `D27`. The TOC references regions and named payloads; it is not a hard-coded fixed address.

The layout is intended to be deterministic and directly addressable, not a 3D graphics scene. A third, Z dimension can represent discrete historical layers or slices.

## 3.6 Z-axis history and provenance overlays

The workbook can add a third, discrete Z axis for history. This is not a 3D rendering problem: each Z position is a slice or layer in a sequence, analogous to a stack of thin sheets. X and Y identify cells in the raster; Z selects a historical layer.

In the current file format, each worksheet's base cells are Z=0. A top-level `<history>` element stores consecutive per-sheet layer deltas using ordinary `<cell>` entries for additions or replacements and `<delete ref="..." />` tombstones for removals. The reader validates sheet IDs, Z order, and duplicate coordinates. This is the simple file-based AI exchange: the AI submits a workbook containing a proposed layer, and Lotus composes it for inspection. Author identity, signatures, and conflict resolution are not yet part of the format.

History is represented as overlays, not as a requirement to duplicate a complete workbook snapshot at every layer. Moving down a layer overlays the earlier state onto the current XML-style representation. Unchanged objects can remain as they are; changed objects expose their historical state; objects created or modified at a newer level must not be accidentally replaced by older content. Provenance metadata establishes the relative historical position of each object or change.

Because every cell carries metadata including its name, overlays can match named objects and evaluate provenance instead of relying only on grid coordinates. Coordinates can change as the raster expands or objects move; names and provenance allow the system to identify the corresponding object and version. The implementation must define the identity scope, provenance ordering, and conflict rules.

Deletion must be represented explicitly (for example, by a tombstone or equivalent provenance-bearing marker), so that absence can be distinguished from no change and historical restoration remains possible. The overlay rules should make it clear which version is visible at a selected Z layer and how newer objects remain visible when traversing older layers. This model preserves continuity without storing a full duplicate of every unchanged payload at every historical step.

## 3.7 Schema evolution

The document format should include an explicit version and a policy for extension.

Future versions may introduce new formula functions, document types, metadata fields, or resource encodings. An older reader should be able to distinguish unsupported features from malformed data.

Namespaces or another explicit extension mechanism can allow independently developed features without silently changing existing semantics.

Compatibility should be a documented property of the format, not an assumption that every reader understands every extension.

# 4. Spreadsheet Calculation Engine

The spreadsheet engine is the computational foundation of Lotus 5-7-11.

A grid that merely displays and edits values would not satisfy the project. The engine must evaluate formulas, understand references, track dependencies, and update results correctly.

## 4.1 Cell data model

A cell should be able to represent distinct categories of information, including:

- Numeric values.
- Text.
- Boolean values.
- Empty cells.
- Formula expressions.
- Calculation errors.
- Dates or other explicitly defined logical types.
- References to other workbook objects, where appropriate.

The internal representation should distinguish a formula from its last calculated value. A formula cell has an expression that defines how its result is produced; the cached result is a consequence of executing that expression.

This distinction matters for recalculation, serialization, debugging, and AI inspection.

## 4.2 Formula language

The formula language should provide the capabilities expected from a useful modern spreadsheet.

At a minimum, the specification should cover:

- Arithmetic operators and precedence.
- Comparisons and Boolean operations.
- Cell and range references.
- Relative, absolute, and mixed references.
- Conditional expressions.
- Aggregation functions.
- Lookup operations.
- String manipulation.
- Numeric and logical functions.
- Error handling.
- Named ranges and named expressions.
- Explicitly defined iteration behavior where needed.

The implementation should not blindly inherit Excel's entire function catalog or undocumented edge cases. It should establish a clear specification, then expand deliberately.

Compatibility with historical Lotus behavior can be introduced where it provides value, but the new engine should have well-defined semantics of its own.

## 4.3 Dependency tracking

Formula evaluation should be supported by a dependency graph.

If cell `C1` depends on `A1` and `B1`, the engine records those relationships. A change to `A1` can invalidate the affected computations and trigger recalculation of dependent cells.

This permits incremental recalculation instead of reevaluating the entire workbook after every edit.

The dependency model should support cycle detection and explicitly defined behavior for circular references. If iterative calculations are supported, their convergence criteria, iteration limits, and failure behavior must be specified.

The same dependency infrastructure can eventually track changes affecting embedded reports, computed documents, and other derived objects.

## 4.4 Scientific calculator and graphing engine

Scientific and graphing calculator capabilities are native spreadsheet functions, not a separate application bolted onto the grid. The engine should include trigonometric and inverse functions, logarithms, exponentials, powers, roots, statistical functions, matrix/vector operations, and other standard scientific calculations. Numeric semantics—including floating-point behavior, precision policy, angle modes, and reproducibility—must be documented.

Graph objects should reference cells, ranges, or expressions directly and participate in the common dependency model. When source data changes, the graph updates through the same invalidation and recalculation system as formulas. Useful initial graph types include Cartesian plots, scatter plots, multiple data series, and statistical plots, with further modes added deliberately.

Where practical, the engine can add numerical methods such as regression, curve fitting, root finding, and numerical integration. Unit metadata and dimensional checks may help prevent invalid scientific operations, but their semantics must be explicit rather than inferred from display formatting.

## 4.5 Deterministic execution

Given the same workbook state, defined inputs, and execution settings, the engine should produce reproducible results wherever its operations permit it.

The specification must account for floating-point arithmetic, calculation order, date and time functions, random functions, and other sources of variation.

Functions that depend on external state should be distinguishable from pure calculations. This matters both for reproducibility and for AI-generated procedures that need to be evaluated or reviewed before execution.

# 5. Embedded BASIC Programming Environment

Spreadsheet formulas are effective for expressing relationships between values. They are less convenient for arbitrary sequences of operations, loops, multi-step algorithms, and procedural workflows.

An embedded modern BASIC dialect would complement the formula engine.

## 5.1 Role of BASIC

BASIC would provide:

- Variables and data types.
- Conditional statements.
- Loops and iteration.
- Procedures and functions.
- Error handling.
- Access to cells and ranges.
- Workbook manipulation operations.
- Controlled interaction with documents and resources.
- Automation of repetitive spreadsheet tasks.

The runtime should be part of the application rather than requiring an external scripting service. Its primary purpose is to supply general control flow and Turing-complete computation to complement dependency-driven formulas, not to turn the workbook into an unrestricted system scripting host.

The goal is not necessarily to reproduce an old BASIC dialect exactly. A modernized dialect could retain familiar syntax while adding structured programming, modules, clear scope rules, and stronger error handling.

## 5.2 Shared execution model

BASIC procedures and formulas should operate on the same underlying workbook model.

There should not be separate spreadsheet states for the formula engine and the BASIC runtime. Both should interact with the same defined cell values, expressions, dependencies, and workbook objects.

However, they need not share identical execution semantics. Formula evaluation can remain expression-oriented and dependency-driven, while BASIC executes ordered procedures.

The interface between them must specify when values are read, when writes become visible, when recalculation occurs, and how errors propagate.

## 5.3 Transactional operations

A useful design would allow a BASIC procedure to stage changes and commit them as a defined operation.

For example, a procedure that advances a simulation should not overwrite values that are still needed to compute the current generation.

The runtime could support explicit transactions or staged writes:

1. Read the current workbook state.
2. Calculate proposed changes.
3. Validate the results.
4. Commit the changes.
5. Recalculate affected dependencies.
6. Expose the resulting state.

This is especially useful for simulations, batch edits, and AI-generated operations.

## 5.4 Portable, self-contained execution

The BASIC interpreter should be implemented in C or Rust and built into Lotus 5-7-11, with no external runtime or third-party dependencies required by the language engine. Platform primitives required by the application itself are a separate concern. A compact interpreter or bytecode VM is sufficient; a JIT and a large runtime are not required.

BASIC exists primarily to provide general control flow and Turing-complete computation. Its execution authority is nevertheless narrow: a procedure is confined to its assigned cell or cells and cannot reach outside that scope merely by naming another address. Reads and writes must be enforced by the runtime, not trusted to program discipline. Any cross-cell access, if supported by a particular operation, must be represented by explicit scope or an authorized reference.

The runtime should impose execution and resource limits, including instruction budgets, memory bounds, cancellation, and recursion limits. Cell confinement limits reachable state; it does not by itself prevent an infinite loop. BASIC does not receive general filesystem, shell, process, network, FFI, or native-extension access.

The dialect and runtime must retain the control flow and state needed for general computation within the model's defined execution scope. The fact that the interpreter is written in C or Rust does not determine the language's computational power.

# 6. Embedded Documents and Cross-References

One of the most significant extensions beyond a conventional spreadsheet is the ability to include other documents in the same logical artifact and establish references between those documents and spreadsheet cells.

The workbook becomes a document graph rather than a flat collection of worksheets.

## 6.1 Documents as first-class objects

The logical document model can contain or reference supporting materials such as:

- Plain-text notes.
- Markdown documents.
- Specifications.
- Instructions.
- Reports.
- Structured records.
- Formula documentation.
- Program documentation.
- Machine-readable descriptions of workbook objects.

Each embedded document should have a stable identity, a declared content type, and defined rules for references.

A document should not need to be reduced to a cell comment or copied into an unrelated attachment merely because it provides context for a calculation.

## 6.2 Cell-to-document references

A cell could reference a document that explains its meaning, records its source, or describes the rules governing its value.

For example, a cell representing a simulation's initial population might link to a specification describing how that population was chosen.

A user could navigate from the cell to its documentation. An AI could follow the same reference to retrieve the context required to modify the model correctly.

## 6.3 Document-to-cell references

References should also work in the opposite direction.

An embedded report could refer to a named range, cell, or formula and display its current value. A specification could identify the cells that constitute its inputs. An explanation could link directly to the procedure responsible for a particular result.

This makes supporting documentation part of the working model rather than a detached description that can silently become outdated.

## 6.4 Live references and snapshots

The format should distinguish between two important cases.

**Live references** resolve against the current workbook state. If a referenced value changes, a dependent report may need to be refreshed.

**Snapshots** preserve a value or state as it existed at a particular point in time. They are useful for reports, decisions, reproducibility, and audit records.

A live reference and a snapshot must not be confused. A historical report should not silently change simply because an input cell was edited later.

## 6.5 Dependency propagation

Cross-document references can participate in dependency tracking.

If a report incorporates a calculated value, the engine can record that dependency. If a specification refers to a procedure, the document model can maintain a navigable relationship between them.

Not every textual mention should automatically become a live dependency. The schema should distinguish explicit references from ordinary prose so that the system can determine which changes require recomputation or notification.

# 7. Binary Resources and Embedded Data

The binary workbook blob can contain arbitrary binary payloads directly, without Base64 conversion or a separate packaging layer. Payloads can be assigned to named cells and consumed in place. The logical XML-style model describes their identity, type, references, and other metadata; it does not require payload bytes to be text.

Possible payloads include:

- Images.
- Small lookup tables.
- Embedded data samples.
- Compact serialized structures.
- Other explicitly supported binary payloads.

A resource should have an identifier, declared media type, encoding, and appropriate integrity information.

## 7.1 Direct binary storage and access

Because the native container is binary and uncompressed, payloads do not need Base64 expansion, compression/decompression, or individual-file packaging. A program that understands the workbook format can locate a named cell and consume its bytes directly. For example, an XML parser, image decoder, or interpreter can operate on a suitable memory buffer or byte range without exporting a temporary file.

A single large workbook can therefore be copied or transferred as one blob rather than as a directory of many small files. This does not make every operation automatically faster: random access and small updates depend on the index, payload layout, and write strategy. The format should support efficient lookup and bounded updates without requiring a full rewrite for every cell edit.

## 7.2 Resource references

Cells, documents, and programs should be able to refer to resources by stable identifiers.

For example, a cell could identify an image, a document could reference a dataset, or a procedure could access a declared binary resource through the workbook API.

The resource model must define ownership, lifetime, missing-resource behavior, and whether a reference is embedded or external.

# 8. Human–AI Collaboration Model

The AI integration is not an ornamental feature layered onto a spreadsheet. It is one reason for designing the document format around explicit structure and executable semantics. Lotus 5-7-11 remains useful as an independent end-user tool; AICO does not need the raster workspace when already operating in its native vector semantic latent space.

A conventional text prompt communicates instructions through natural language. The recipient must infer the relevant state, interpret the intended operation, and determine what information matters.

A structured workbook can make much of that information explicit.

## 8.1 The workbook as shared state

The workbook serves as a persistent working state that a human and an AI can inspect and modify over successive turns.

It can contain:

- Current inputs and outputs.
- Named variables and ranges.
- Constraints and validation rules.
- Supporting documentation.
- Procedures and formulas.
- Intermediate results.
- References to relevant objects.
- Optional history and provenance.

The AI does not need to treat every interaction as an entirely new textual description of the problem. It can work against the current artifact. JSON may be useful as an interchange or command transport format, but it is not itself the working environment; the workbook supplies persistent state, executable semantics, inspectable objects, and an interactive surface.

This is analogous to a chessboard. Participants share a defined state, perform operations against it, and inspect the resulting position before the next turn.

The workbook file is the AI-facing interface. The AI submits proposed changes as a new Z-layer update in that file; Lotus reads and validates the update, then makes the resulting layer available for inspection or composition. This does not require a separate AI API. Z-layers also preserve the previous workbook state so a change can be reviewed and reversed.

## 8.2 Turn-based interaction

A useful interaction cycle would be:

1. The human establishes or modifies the workbook.
2. The AI inspects the workbook file and its documented semantics.
3. The AI submits a proposed Z-layer update in the workbook file.
4. Lotus validates the update and records it as a new layer.
5. The resulting view is recalculated and can be inspected against earlier layers.
6. The human decides whether to keep, revise, or reverse the change.

The AI may communicate in natural language when explanation is useful, but natural language need not be the only representation of its instructions.

A proposed operation could identify the affected cells, reference a procedure, or supply a formula transformation. The workbook provides the context against which the operation is interpreted.

## 8.3 Structured intent

A workbook can express more than values.

It can represent the meaning of values, the relationships between them, the constraints governing them, and the permitted operations on them.

For example, a budget model could explicitly identify input cells, units, derived totals, and constraints. An AI asked to adjust the budget could inspect those definitions rather than infer everything from cell positions and surrounding prose.

Similarly, an engineering simulation could expose its parameters, invariants, calculation procedures, and expected outputs as structured objects.

This does not eliminate ambiguity from every task. It reduces ambiguity where the relevant information can be represented explicitly.

## 8.4 Formula and procedure generation

The AI can generate formulas for local relationships and BASIC procedures for multi-step operations.

The spreadsheet engine evaluates formulas according to its specification. The BASIC runtime executes procedures according to its defined semantics.

This division is valuable because it separates the generation of instructions from their execution.

A model may propose an incorrect formula or procedure. The engine should not assume that generated code is correct merely because it is syntactically valid.

Validation, execution permissions, error reporting, and review remain part of the design.

## 8.5 The artifact as an interface

The same workbook can be used through the ordinary user interface and directly as the AI exchange artifact.

The user can edit a cell manually. An AI can submit a Z-layer that changes the same logical value. Both interact with the same workbook model and see the resulting state.

The AI exchange does not grant access beyond the workbook. Embedded procedures cannot grant additional authority: runtime execution remains inside workbook-resident state. Any unrelated tools supplied by the surrounding host are governed by that host.

This makes the spreadsheet an ordinary productivity tool and a structured collaboration interface without requiring separate representations of the underlying work.

# 9. Interface and Interaction Design

The initial interface should retain the immediacy of a traditional spreadsheet.

Users should be able to navigate the grid, edit cells, inspect formulas, define ranges, and invoke procedures without needing to understand the XML representation.

The interface should also make the richer document model accessible without overwhelming ordinary spreadsheet use. The design should recover much of Lotus 1-2-3's interaction model and menu workflow, including keyboard-only control. Keyboard interaction is a first-class path, not an accessibility afterthought; mouse support is additive. Commands should be discoverable through hierarchical menus, with predictable focus, selection, cancellation, and command-entry behavior.

The application should also provide scientific and graphing calculator capabilities as native spreadsheet features: scientific functions, statistical analysis, matrix/vector operations, and graphs that reference cells, ranges, or expressions and update through the common dependency model.

Possible facilities include:

- A conventional worksheet grid with keyboard-first navigation and editing.
- Formula inspection and editing.
- A procedure editor for BASIC.
- Navigation between cells and associated documents.
- Inspection of dependencies.
- Validation and error reporting.
- A history of defined operations.
- A structured view of the workbook for advanced users and AI tooling.
- A command-menu workflow inspired by Lotus 1-2-3, with full keyboard-only operation.
- Native scientific-calculator and graphing views.
- A generative UI layer that lets AI construct and modify live workbook objects rather than merely emit screenshots or bespoke application code.

These should be views of the same underlying document, not independent applications that maintain incompatible copies of its state.

## 9.1 The workbook as a generative UI runtime

The workbook can act as a basic generative UI for AI. The AI constructs or modifies native objects—tables, input controls, formulas, charts, documents, and procedures—through a defined structured operation interface. Lotus 5-7-11 supplies the renderer, calculation engine, object model, and interaction behavior. The generated interface is live workbook state, not a picture or a throwaway front end.

A generated comparison tool, scientific workbench, planning surface, or data-entry interface can be saved, inspected, edited by a human, reused, and extended after the AI's turn ends. The same objects are accessible through the keyboard-first human interface and through Z-layer updates exchanged in the workbook file. Presentation does not grant authority: controls request operations, while the runtime and workbook rules enforce what is permitted.

The interface can be modernized without abandoning the compactness and direct manipulation that made the original spreadsheet model effective.

# 10. Reverse-Engineering the Original Lotus Implementation

Disassembling an original Lotus binary is a potentially valuable starting point.

The objective would be to understand the implementation's behavior and computational model, not necessarily to reuse its code.

## 10.1 Establish the target

The specific Lotus release and platform must be identified before detailed analysis begins.

Different releases may use different executable formats, processor architectures, runtime environments, and internal implementations.

The original binary, supporting files, and test environment should be preserved so that observations can be reproduced.

## 10.2 Map the architecture

Initial investigation should identify the major subsystems and their relationships.

Areas of particular interest include:

- Cell storage and worksheet representation.
- Formula parsing and evaluation.
- Range operations.
- Dependency tracking and recalculation.
- Numeric representation and error propagation.
- File loading and saving.
- Macro parsing and execution.
- User-interface commands and keyboard interaction.

Static disassembly can help identify candidate routines and data structures. Dynamic tracing and controlled experiments can establish how they behave.

## 10.3 Use the original as a behavioral reference

Construct small workbooks that isolate individual behaviors.

Examples include formulas with different operator precedence, mixed cell references, boundary conditions in ranges, circular dependencies, and recalculation after editing an input.

Record the original program's results and compare them against the replacement engine.

This produces a behavioral specification grounded in observation rather than assumption.

## 10.4 Separate compatibility from modernization

The investigation should distinguish behavior worth preserving from implementation mechanisms that can be replaced.

The goal is not to recreate old memory management, platform assumptions, or architectural limitations. The objective is to identify the computational and interaction semantics that remain valuable.

The new implementation should be free to use a different internal architecture while preserving selected externally observable behavior.

Any plan to distribute code derived from the original requires separate consideration of copyright, licensing, and other applicable legal restrictions. Behavioral study and code reuse are distinct matters.

# 11. Implementation Language and Portability

C and Rust are both viable candidates for the implementation.

The decision should be based on the desired runtime characteristics, memory-management strategy, portability requirements, and development constraints rather than on the assumption that either language guarantees a minimal executable.

## 11.1 C

C provides direct control over memory allocation, data structures, and platform interfaces. It is a natural fit for a small runtime with carefully bounded responsibilities.

Its principal cost is that memory safety, resource ownership, and error handling must be enforced through disciplined design and testing.

## 11.2 Rust

Rust offers stronger compile-time guarantees around memory safety and data races while supporting low-level programming.

It can be used to build a portable application, but the dependency-free requirement still needs deliberate implementation choices. The standard library, target platform, and any third-party components must be considered separately.

## 11.3 Dependency policy

Portability should be an architectural requirement rather than a marketing claim.

The project should define what counts as an acceptable dependency and which platform facilities are permitted.

The XML parser, formula evaluator, BASIC runtime, and core workbook model should have narrow interfaces. The UI and operating-system integrations should not be prerequisites for using the document format itself.

The core engine should be testable without launching the graphical interface.

A standalone executable is a useful target, but portability also requires documented build procedures, supported targets, and a stable specification.

# 12. Security and Execution Boundaries

Embedding programs and resources in a document introduces important security considerations.

An XML workbook should not be assumed safe merely because it is text-based or human-readable.

## 12.1 XML parsing

The parser should reject malformed input cleanly, impose appropriate resource limits, and disable unnecessary XML features such as external entity resolution when they are not required.

It should also defend against excessive nesting, oversized payloads, and pathological input that consumes disproportionate memory or CPU time.

## 12.2 BASIC execution

Embedded BASIC procedures are permanently confined to workbook-resident state. They may read and write their assigned cells, and any additional cell or embedded-resource access must be explicitly granted by the workbook execution context.

The runtime must never read from or write to the filesystem, network, process environment, or other operating-system services. If a procedure needs outside data, the user first imports that data into the workbook. Exporting workbook content is a separate host-side operation initiated by the user. Runtime capabilities can grant access within the workbook; they cannot grant authority beyond it.

## 12.3 AI-generated operations

AI-generated formulas and programs should be treated as proposed executable content, not trusted instructions.

The application should provide syntax checking, error reporting, and a means to validate proposed changes before committing them. More consequential operations can require explicit human approval.

The engine should enforce its own constraints independently of what the AI claims an operation will do.

The AI must not be able to bypass runtime constraints by placing instructions or code in the workbook. Lotus validates submitted Z-layer updates as workbook changes; embedded procedure calls separately pass through cell-level runtime permissions. Lotus exposes no filesystem, network, process, or operating-system access to the runtime. If the AI runs in an environment with other tools, Lotus's workbook boundary does not replace the host's controls over those tools.

## 12.4 Integrity and provenance

The format may support hashes, revision metadata, provenance, and records of defined operations. Each named cell already carries metadata including its name. Provenance and stable object identity can establish which historical version of a named object is being viewed, while coordinates identify its current raster location. Names should not be assumed globally unique unless the schema explicitly makes them so.

These features should have explicit semantics. A hash can detect changes relative to a known value, but it does not by itself establish authorship or authorization.

The design should avoid implying security guarantees that the implementation does not actually provide.

# 13. Development Strategy

The project should proceed in stages so that the document model and computational engine become useful before the entire application is complete.

## Stage 1: Behavioral research

Identify the original Lotus target, inspect its major subsystems, and establish a reproducible set of formula and interaction tests.

Deliverable: an initial behavioral specification and test corpus.

## Stage 2: Native blob and logical document specification

Define the uncompressed binary blob layout, XML-style logical schema, cell-name metadata, TOC placement, byte-range lookup, object identity, reference semantics, and versioning strategy. Define the distinction between the logical XML representation and the native binary serialization.

Deliverable: a documented format with sample workbooks, direct payload access, export/import round-trip tests, and validation tests.

## Stage 3: Minimal spreadsheet engine

Implement cell storage, formula parsing, evaluation, range operations, dependency tracking, recalculation, and serialization.

Deliverable: a headless engine capable of loading, modifying, calculating, and saving workbooks.

## Stage 4: Interactive grid

Build the user interface around the engine, with ordinary spreadsheet navigation and editing.

Deliverable: a usable local spreadsheet application.

## Stage 5: BASIC runtime

Implement the self-contained C or Rust interpreter/VM, control flow, local state, assigned-cell confinement, resource limits, and staged writes.

Deliverable: a portable, dependency-light, Turing-complete procedural runtime whose authority is constrained by workbook scope.

## Stage 6: Document graph

Add embedded documents, stable references, linked values, resource management, and dependency-aware navigation.

Deliverable: a unified workbook containing data, logic, and supporting documentation.

## Stage 7: Human–AI and generative UI interface

Define structured operations for inspecting workbook state, constructing live UI objects, proposing changes, invoking procedures, and reviewing results. Implement the Lotus-inspired menu workflow and keyboard-only operation alongside mouse support.

Deliverable: a workbook that supports explicit, stateful, turn-based human–AI collaboration and AI-generated interfaces composed of native objects.

## Stage 8: Compatibility and hardening

Expand the formula test corpus, test malformed documents and hostile programs, verify portability, and refine serialization and execution behavior.

Deliverable: a stable implementation with documented compatibility guarantees.

# 14. Evaluation Criteria

The project should be evaluated against measurable capabilities rather than the number of features listed in a specification.

### Document format and blob storage

- Is the native workbook one uncompressed binary blob?
- Can named cell payloads be read directly without extracting files?
- Can a cell be ejected with its bytes preserved exactly?
- Can the TOC be located from the maximum used X/Y extents?
- Are the logical XML model and binary serialization clearly distinguished?
- Are references and provenance stable and unambiguous?
- Can unsupported extensions be identified safely?

### Spreadsheet engine

- Are formula semantics specified?
- Are dependencies recalculated correctly?
- Are errors reported consistently?
- Can large workbooks be processed without unnecessary full-document materialization?

### BASIC runtime

- Is the interpreter implemented in C or Rust without third-party runtime dependencies?
- Does BASIC provide general control flow and Turing-complete computation within its defined scope?
- Are procedures confined to their assigned cell or cells?
- Are reads, writes, and commits well-defined?
- Can execution be bounded and interrupted safely?
- Can the same procedure be tested independently of the UI?

### Embedded documents

- Can documents reference cells and other objects?
- Can live references be distinguished from snapshots?
- Can dependencies be identified when referenced values change?

### Human–AI collaboration and UI

- Can an AI inspect relevant workbook structure without relying exclusively on prose?
- Can it construct a live interface from native workbook objects?
- Can it propose a precise change to a defined object?
- Can the application validate and report the outcome?
- Can a human operate the application entirely by keyboard?
- Can a human inspect, modify, and continue from the same state?

### Portability

- Can the core engine be built and run on the intended target platforms?
- Are external dependencies documented?
- Can the native document be inspected and processed without launching the full application?

# 15. Conclusion

Lotus 5-7-11 is a proposal for a modern computational spreadsheet built around one uncompressed binary workbook blob, an XML-defined logical document model, a formula engine, and a self-contained embedded BASIC runtime.

Its distinguishing feature is the relationship between the raster workspace and the broader document model. A cell is a named blob whose payload can be a file, program, document, or other arbitrary byte sequence; ejecting the cell produces the standalone file. The workbook remains one blob, and compatible software can consume payloads directly without using a filesystem. The X/Y grid provides discrete workspace addressing, while Z provides discrete historical layers ordered by provenance.

This has practical value for ordinary users. It also provides a structured medium for AI collaboration: the human and the machine can work against the same persistent artifact, inspect its state, perform defined operations, and examine the results.

The original Lotus implementation offers an opportunity to study a mature computational model. Reverse engineering can inform the replacement, while a new implementation in C or Rust can adopt modern memory management, portability, and execution boundaries.

The central design objective is not to reproduce the complexity of a contemporary office suite. It is to build a compact, portable computational workspace with Lotus 1-2-3-inspired keyboard interaction, scientific and graphing capabilities, a constrained Turing-complete BASIC runtime, native blob storage, layered provenance history, and a generative UI that works for humans and AI alike.

**Lotus 5-7-11 would bring the programmable spreadsheet forward by making the document itself a clear representation of data, logic, relationships, and intent—usable by humans and directly tractable by machines.**

## Addendum — Native Executable Format and Startup Settings (2026-10-14)

The native Lotus 5-7-11 file is intended to be executable in its own right: the application is part of the native format, rather than a separate program required to interpret an otherwise passive workbook. The application may still open and operate on other files; those files do not need to be executable. A size target of roughly one floppy's capacity for the program itself would support compact, portable distribution across compatible x86 environments, subject to the executable/boot convention and host execution interface.

The file layout is deliberately inverted. The program occupies one end of a 64-bit address space and the workbook container the other, with a compressed chasm between them. The chasm reduces physical footprint and also defines structural relationships; this layout is intended to support representational fluidity, not merely compression. The exact mapping and invariants remain part of the format design.

Immediately before the program is a settings-cell region that serves as a file-local startup sequence, analogous in role to `AUTOEXEC.BAT`. Most entries are predetermined options, but the region can also contain bounded binary settings data. Because the layout is upside down, binary written into this region must be reversed to match the format's orientation. These settings may influence initialization and operation, including compatibility choices, but their authority is confined to the file: they must not provide a path for code or data to escape into unrestricted host-system access.

Compatibility is to be selected at the command, job, or workflow level rather than by declaring one spreadsheet system universally authoritative. Where Excel 2003 and Lotus 1-2-3 differ, Lotus 5-7-11 should favor whichever behavior is more robust for the operation at hand, while keeping its native calculation and execution rules explicit. Formula syntax, calculation semantics, and macro execution remain distinct compatibility layers; embedded BASIC is the native programmable language, not an attempt to reproduce VBA or every legacy Lotus macro facility.
## Addendum — Self-Contained x86 Environment and Generative Terminal UI (2026-10-14)

Lotus 5-7-11 is intended to be self-contained and to look and behave substantially the same across supported x86-64 execution environments. It is not designed around Windows or Linux application frameworks. Its primary interface is keyboard-driven and terminal-native; mouse support is not required. The application carries at least a small set of its own fonts and defines its own palette, including 256 colors with light and dark variants. Cell widths and heights are resizable, and cell borders can be styled or suppressed. These are native layout and rendering properties rather than dependencies on a host desktop UI toolkit.

The intended core is compact: the spreadsheet calculation engine and embedded BASIC runtime are the principal substantial components. BASIC and spreadsheet calculations should share numeric and other common primitives where practical. Graphing is a separate, on-demand tool rather than a permanently resident part of the core. A core executable footprint around 256 KB is a design target to measure during implementation, not yet a demonstrated binary size; actual size will depend on the implemented function set, runtime support, rendering path, and build configuration.

The native artifact includes the program and workbook representation in the same inverted binary format. The settings-cell region immediately before the program can influence startup within defined bounds, but it cannot modify the compiled program; changes to the program must be made before compilation. The file's mask accounts for host-dependent entry stubs that may be skipped or selected through low-level format handling. These stubs are transfer points to one canonical program entry point, not independent implementations of the application. Each supported path must establish or normalize the initial execution state required by that common entry point.

The memory map is the principal abstraction boundary. The application is written against its defined execution model rather than against Windows or Linux APIs. Host- or environment-dependent details are concentrated at the execution handoff and at the interfaces the memory map exposes. A compatible loader or execution substrate must still establish the required processor state and provide the input, display, storage, and memory interfaces the program expects; x86-64 instruction compatibility alone does not supply a universal hardware interface. When launched as a unikernel under Bare Metal OS, the intent is to reuse the substrate's existing contracts and avoid unnecessary general-purpose operating-system housekeeping.

The same cell primitives also form a basic generative UI. A generated interface can be assembled from named cells whose content, dimensions, colors, fonts, and border rules already have meaning to the spreadsheet. Panels, forms, status displays, menus, dashboards, and similar interfaces can therefore be created by manipulating native workbook objects rather than introducing a separate GUI object system. This is expected to be less fluid than a purpose-built graphical toolkit for animation-heavy applications, but it is a direct fit for technical workspaces, data displays, configuration tools, and AI-generated interfaces. UI description and executable behavior remain separate concerns, with BASIC's runtime-enforced cell-access boundaries continuing to constrain what procedures can affect.

Because the execution contract is not specific to Lotus, the same environment is also intended to provide a base interface for launching `ggml` as a unikernel. Lotus is a consumer and practical test of the shared substrate, not a required dependency of the inference engine. The common foundation is the memory-map and execution contract: a self-contained program, one canonical entry point, narrowly scoped environment-dependent handoffs, and defined interfaces to memory and I/O.
