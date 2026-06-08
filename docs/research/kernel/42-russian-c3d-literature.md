# 42. Russian-Language Literature on CAD Geometry Kernels (C3D / Golovanov / ASCON)

## Title and Scope

This dossier is a deliberate Russian-language literature pass over CAD geometry-kernel technology, translated into English, aimed at one question: does the Russian tradition contain engineering knowledge about B-rep solid modeling that is absent or thin in Keel's existing ~41-dossier English-language corpus? The Russian tradition is the natural place to look, because C3D (the kernel from C3D Labs, a spin-off of ASCON) is the only fully independent non-Western production kernel, cited throughout our corpus as the robustness benchmark with its "500,000+ model" regression suite (see dossier 29, 34). Its intellectual base is Russian, above all Nikolay Golovanov's book *Geometric Modeling* (Геометрическое моделирование), and a decade of C3D Labs engineering blog posts on Habr and sapr.ru.

The themes covered: Golovanov's book and its architecture; C3D Modeler engineering (tolerant geometry, booleans, data model); the C3D Solver; the C3D Converter; the ASCON/KOMPAS lineage; and the Soviet/Russian applied-geometry and spline traditions. Each source gets a citation, translated content, a novelty verdict, and a kernel-relevance note.

## Method and the English-Corpus Baseline

Method: searches were issued in Russian (Cyrillic) against web search; Russian-language pages were fetched and key passages translated faithfully into English, with the original Russian term of art quoted in parentheses on first use. Where a finding rests on a secondary summary rather than a primary fetched page, that is stated. I did not obtain a full-text PDF of Golovanov's book; the book's content here is drawn from its published table of contents (the 2024 КУРС/ИНФРА-М edition, and the 2002 Fizmatlit first edition) plus the C3D Labs blog corpus written by the same authors. Two C3D pages (the fizmatlit TOC mirror, the newest 2025 dev article) were partially or wholly unreachable and are flagged.

The English-corpus baseline that matters for the novelty verdict:

- Our corpus already knows C3D as a robustness *corpus* benchmark (the 500k models), but treats that as an organizational asset, not an algorithmic one (dossier 29, 34). It did **not** mine the C3D engineering blogs for *algorithmic* technique.
- Tolerant modeling is already covered in the English corpus from the ACIS/Parasolid tradition: tolerant edges and vertices, "tolerant booleans," fuzzy/snapping tolerances (dossiers 13, 17, 29, 30, 39). The canonical English references are Jonathan Corney / T. Lim, the ACIS documentation, and the SMLib/Solid Modeling Solutions material.
- The single most important honesty fact for this whole pass: **Golovanov's book has an authorized English translation**, *Geometric Modeling: The Mathematics of Shapes*, ISBN 978-1497473195, circa 2015. So the book's content is, in principle, already inside the reach of an English-language corpus. The "Russian gap" is therefore not the book itself; it is (a) whether our corpus actually absorbed the book, and (b) the C3D Labs *blog* engineering writing, which is Russian-only and was not in our corpus.

## Theme 1: Golovanov, *Geometric Modeling* (Геометрическое моделирование)

### Source 1: Golovanov, *Geometric Modeling*, table of contents (2024 edition)

**Citation.** N. N. Golovanov (Н. Н. Голованов), *Geometricheskoe modelirovanie* (Геометрическое моделирование, "Geometric Modeling"), Moscow: КУРС / ИНФРА-М, 2024, 400 pp., ISBN 978-5-905554-76-6. First edition: Fizmatlit, 2002, 472 pp. English translation: *Geometric Modeling: The Mathematics of Shapes*, 2015, ISBN 978-1497473195. TOC fetched from znanium.ru catalog (https://znanium.ru/catalog/document?id=439456); book pages: https://urss.ru/cgi-bin/db.pl?page=Book&id=10999 ; https://c3dlabs.com/company/book/ .

**Content (translated).** Golovanov is the chief architect of C3D, so this book is effectively the design rationale for a shipping Parasolid-class kernel, not an academic survey. The structure:

- Ch. 1 Points (Точки): radius vectors, affine and homogeneous coordinates.
- Ch. 2 Curves (Кривые): analytic curves, point-fit curves, Bezier, rational Bezier, B-splines and the de Boor algorithm, composite curves (составные кривые).
- Ch. 3 Surfaces (Поверхности): analytic and motion-swept surfaces, grid and point surfaces, Bezier, B-surfaces, T-surfaces, triangular Bezier and simplex splines (симплексные сплайны), deformable and boundary (filling) surfaces.
- **Ch. 4 Projections and Intersections (Проекции и пересечения):** point projection onto curves/surfaces; curve-curve, curve-surface, surface-surface intersections; intersection-curve representation and algorithms; **blend and chamfer surfaces (4.9-4.10)**; and **4.11-4.13 point-classification testing and tolerance (положение точки; погрешность/толерантность)**.
- **Ch. 5 Solids (Тела):** shell and solid definitions, **5.5 the data structures (структуры данных)**, elementary solids, construction methods.
- **Ch. 6 Methods of building solids (Методы построения тел):** overview, **6.2-6.3 Boolean operations on solids (булевы операции над телами)**, cutting/section, symmetry, feature ops, offsets, thin-shell (оболочка), fillets, chamfers, and **direct modeling (прямое моделирование)**.
- Ch. 7 Geometric constraints (Геометрические ограничения): constraint formulation and solution methods.
- Ch. 8 The geometric model (Геометрическая модель): model composition, visualization, **triangulation methods (8.6-8.8)**, inertial/mass properties.
- Appendix: curvilinear coordinate systems, tensors.

The book explicitly frames boundary representation (граничное представление) as the standard for modern CAD: "description of the geometric form of the modeled object by curved faces meeting one another along common edges."

**Novelty verdict: (b) a deeper / single-source Russian treatment of material the English corpus already has, made (a)-adjacent by one detail.** The chapter map is essentially the Parasolid/ACIS feature set; topology (Ch. 5), booleans (Ch. 6), intersection (Ch. 4) are all things our corpus covers. The book's distinctive value is that it is a *single self-consistent derivation* of an entire shipping kernel by its architect, including the numerical-method and differential-geometry foundations under each operation, which the English literature usually splits across Piegl-Tiller (NURBS), Hoffmann (solid modeling), and vendor docs. The (a)-adjacent detail: sections 4.11-4.13 fold **tolerance/error directly into point classification** (the in/on/out test), i.e. Golovanov treats the tolerance not as a kernel-global epsilon but as a parameter of the classification predicate. That is the same instinct our predicate dossiers (11, 37) reach for, expressed in 2002.

**Kernel relevance.** High as a *reference architecture*. For Keel, the actionable point is that the canonical Russian text already organizes a kernel exactly as our dossiers do, and the English translation exists, so it should be cited as a primary architecture reference in dossier 00/08, not merely as "the C3D book." It does not add a technique we lack; it validates the shape of the design.

### Source 2: Golovanov, *Geometric Modeling*, publisher and English-edition description

**Citation.** ASCON / C3D Labs press, "Nikolay Golovanov's Geometric Modeling Now Available Everywhere," https://c3dlabs.com/blog/company/nikolay-golovanov-s-geometric-modeling-now-available-everywhere/ ; Amazon listing ISBN 978-1497473195; isicad review, https://isicad.net/articles.php?article_num=17461 .

**Content (translated/summarized).** The English edition is described by the publisher as a revised and updated version of the Russian first edition, distilling more than 20 years of developing C3D (used by KOMPAS-3D, BAZIS, ESPRIT, K3, Techtran, others). It "describes the algorithms and data structures of geometric objects" and "principles of interconnection between elements of a model."

**Novelty verdict: (c) redundant for content, but load-bearing for honesty.** This source's value is purely the confirmation that the book is in English. It removes the temptation to claim the book as a "hidden Russian source."

**Kernel relevance.** Procedural: cite the English edition.

## Theme 2: C3D Modeler Engineering (the Russian-only blog corpus)

This is where the genuine Russian-only material lives. These posts are written by the C3D Modeler team leads and are not, to my knowledge, mirrored in English at this depth.

### Source 3: Tumanin, "Trends in the development of the C3D Modeler kernel" (2024)

**Citation.** A. Tumanin (А. Туманин, Head of C3D Modeler development, Ph.D.), "Tendentsii v razvitii geometricheskogo yadra C3D Modeler" (Тенденции в развитии геометрического ядра C3D Modeler), C3D Labs blog, 5 Nov 2024, https://c3dlabs.ru/blog/products/tendentsii-v-razvitii-geometricheskogo-yadra-c3d-modeler/ . (Primary page fetched.)

**Content (translated).** This is the richest single tolerant-modeling source found.

- Precision vs. tolerance (точность vs. толерантность): the kernel uses a small default precision for fundamental geometric tasks; "tolerance" is *relaxed precision* (ослабленная точность), a value exceeding the kernel default, which arises when importing geometry or from operations run at non-default precision.
- **Tolerance is attached to topology, not geometry (толерантность связана с топологией, а не с геометрией).** This is the key sentence. A vertex tolerance is represented as **a sphere centered on the vertex with radius equal to the tolerance** (вершина: сфера с радиусом, равным толерантности); an edge tolerance is represented as **a tube of that radius around the edge** (ребро: труба). The geometry stays exact; the topology carries the slop.
- Why it exists: when two surfaces nearly touch (approach tangency) without truly intersecting, building a single shell requires associating a tolerance with the resulting edges to resolve the topology-vs-geometry conflict and produce a "topologically correct model" (топологически корректная модель).
- New 3D wireframe object MbWireFrame carries both curve geometry *and* tolerance; sweep/kinematic ops accept it as profile or trajectory; fillet, extension, truncation, and point-projection on the wireframe all carry tolerance through the computation.
- Kinematic ops with a dynamically variable cross-section (динамически изменяемое сечение): the section is reshaped under constraints (preserve linear/radial dimensions; tangency/perpendicularity; tangency to external surfaces) as it sweeps.
- Boolean and fillet robustness: improved booleans on regions with precision taken into account; fillets of unconnected faces (скругление несвязанных граней); a **disk-rolling method (метод прокатки диска)** for multi-face filleting along a support curve.
- 2024 planned robustness work: a **ball-rolling method (прокатка шариком)** for fillets without a support curve; a **median shell for non-equidistant face pairs (срединная оболочка для пар неэквидистантных граней)**, computed by point-cloud approximation of the locus of spheres simultaneously tangent to both face sets; and geometric face arrays with automatic trim/extend onto the base shell.

**Novelty verdict: (a) genuinely new at the level of concrete representation, on a topic the English corpus has only abstractly.** Our corpus knows "tolerant edges/vertices" exist (dossiers 13, 29, 30, 39), but it describes them as ACIS does: an edge that is "fat." Golovanov's team gives the **explicit geometric model: vertex = sphere of radius t, edge = tube of radius t, tolerance bound to the topological entity while the underlying curve/surface stays exact.** That sphere/tube formalization is a clean mental and implementation model we did not have written down. The median-shell-as-locus-of-bitangent-spheres construction is also a concrete recipe our midsurface dossier (10) did not state in that form.

**Kernel relevance. High, and directly adoptable.** For Keel's tolerance design this is the cleanest available specification: store tolerance on the topological entity (Vertex, Edge), interpret it as a sphere/tube radius, keep the carried geometry exact, and let booleans/sewing widen tolerance to absorb near-tangency rather than perturbing geometry. This should feed dossiers 29/30/39 and the tolerance model in 17/37.

### Source 4: Tumanin, "C3D Modeler, the basis of the C3D kernel" (2023)

**Citation.** A. Tumanin (А. Туманин), "C3D Modeler - osnova geometricheskogo yadra C3D" (C3D Modeler – основа геометрического ядра C3D), Habr (ASCON blog), 20 Sep 2023, https://habr.com/ru/companies/ascon/articles/762206/ . (Primary page fetched.)

**Content (translated).** Boundary representation (граничное представление) is the primary model, supplemented by polygonal representation (полигональное представление); objects carry a parametric history tree for regeneration. Three classical directions: wireframe, surface, solid modeling (каркасное, поверхностное, твердотельное). Strategic growth: direct modeling, polygonal modeling, sheet metal (листовой металл). The kernel implements **managed accuracy (управляемая точность)** across operations: tolerance at vertices when collecting contours; precision control in curve-to-NURBS conversion; explicit handling of "controlled inaccuracy in operations" (управляемая погрешность) to absorb translation losses. Robustness items: self-intersection diagnostics (диагностика на самопересечение) in sweeps; offset-surface optimization; improved flat-projection (HLR) algorithms; distance measurement between objects (curve-curve, curve-surface). Polygonal-solid hybrid supports mixed geometry in projection with mutual-shadow computation.

**Novelty verdict: (b) deeper, on "managed/controlled accuracy."** The framing that accuracy is *managed per operation* rather than global, and that operations may run at a *deliberately controlled inaccuracy* to absorb import loss, is a more explicit statement of a philosophy our corpus has only implicitly (dossier 37 substrate). Self-intersection diagnostics in sweeps overlaps dossier 41 (blend overflow / feature failure).

**Kernel relevance.** Medium-high. Reinforces a per-operation tolerance policy and an explicit "controlled inaccuracy" budget concept for import-derived bodies.

### Source 5: Kondrikova, "C3D kernel: new functions and directions of development" (2025)

**Citation.** T. Kondrikova (Т. Кондрикова, C3D Modeler group lead), "Geometricheskoe yadro C3D: novye funktsii i napravleniya razvitiya" (Геометрическое ядро C3D: новые функции и направления развития), Habr (ASCON blog), 17 Nov 2025, https://habr.com/ru/companies/ascon/articles/967270/ . (Fetched via search-engine summary plus one successful fetch; one later refetch was connection-refused, so a few specifics are from the summary, flagged.)

**Content (translated).** Booleans on regions improved for tangencies and self-tangencies, and the user can now **control the precision of the boolean operation itself (управлять точностью булевых операций)**, important where tolerances are critical in complex intersections. Tolerant geometry in wireframe ops: edge connectivity in a frame depends on **vertex tolerance (толерантность вершин)** as a rounded precision; equidistant, edge-merge, and shell-intersection ops preserve connectivity (сохранит связность) by **inserting tolerance vertices**. Surface ops: **median shell (срединная оболочка)** as the set of points equidistant from two face sets; surface extension with refined topology; curve wrap/unwrap on developable (zero-Gaussian-curvature) surfaces. Robustness: self-intersection detection in three modes (own-face only / different-face / auto-all); triangulation-based surface diagnostics; **curve smoothness assessed via a potential-energy functional (потенциальная энергия кривой)**. Numerics: parallelization of thin-wall creation and NURBS-surface copy; thread-safe atomic edge retrieval; fixes to intersection-curve concurrency.

**Novelty verdict: (a) on two specifics; (b) elsewhere.** Genuinely useful and not prominent in our corpus: (1) **user-controllable per-call boolean precision** as a first-class API parameter (we treat tolerance as state, not as a per-operation argument), and (2) **inserting tolerance vertices to preserve connectivity** during wireframe edits, i.e. healing connectivity by *adding* tolerant topology rather than snapping geometry. The potential-energy smoothness metric overlaps our fairing work (dossier 32) and is (b).

**Kernel relevance.** High for the API-design point: expose boolean tolerance as a per-operation argument, and model connectivity repair as tolerant-vertex insertion. Feeds dossiers 39 (coincident/tangent booleans) and 13 (healing).

### Source 6: C3D Labs, "Overview of CAD systems on the C3D kernel" (2018)

**Citation.** C3D Labs (c3dlabs), "Obzor SAPR na geometricheskom yadre C3D" (Обзор САПР на геометрическом ядре C3D), Habr, 5 Dec 2018, https://habr.com/ru/companies/ascon/articles/431918/ . (Fetched.)

**Content (translated).** A product/integration overview. Notes that in CAM work the hard part is "coordinating the precision of results obtained through the kernel with the overall precision of the high-level computation" (согласование точности результатов ... с общей точностью вычислений), and that one customer hit difficulties with booleans and curve-on-surface projection due to model specifics. Lists C3D Converter targets: ACIS, IGES, Parasolid, STEP.

**Novelty verdict: (c) redundant.** Marketing-level; the only kernel-relevant nugget (accuracy must be coordinated across kernel and host) is already implicit in our substrate dossier (37).

**Kernel relevance.** Low.

## Theme 3: C3D Solver (the constraint engine)

### Source 7: Alakverdyants, "C3D Solver: principles of parametric 2D patterns and 3D assembly improvements" (2024)

**Citation.** A. Alakverdyants (А. Алаквердянц), "C3D Solver: printsipy parametricheskogo chercheniya 2D-patternov i uluchsheniya dlya 3D-modelirovaniya sborok," Habr (ASCON blog), 9 Apr 2024, https://habr.com/ru/companies/ascon/articles/788338/ . Product page: https://c3dlabs.ru/products/c3d-toolkit/solver/ . (Both fetched.)

**Content (translated).** Constraint categories: logical (логические: tangency, symmetry, coincidence) and dimensional (размерные: angular/linear, patterns). Three operational modes: **controlling (управляющие)** that drive geometry, **informational (информационные)** that only report, and **interval (интервальные)** that allow a value range. The solver also runs **variational (вариационные)** models where all parts in the constraint system are equal-rank, enabling inverse-kinematic solutions. 2D patterns (паттерны) compose rotational and linear symmetries (including combined "carousel" patterns). Spline constraints added length and equal-length; performance for 100+ control-point sketches improved by nearly two orders of magnitude. 3D: interval angular dimension extended to 0-360 degrees; an IsWellDefined query reports complete fixation in "nearly 100% of cases"; constraints carry one of four status values, so the solver detects redundant and contradictory-yet-formally-satisfiable systems (e.g. over-specifying a right triangle by both legs and the hypotenuse).

**Novelty verdict: (b) deeper on diagnostics; (c) on the rest.** Our constraint dossiers (04, 12) already cover variational solving, DOF analysis, well-/over-/under-constrained diagnosis, and interval/driven/driving dimensions. The one sharper detail: the explicit **four-status-per-constraint** classification that flags *contradictory but formally satisfied* systems (a triangle that is numerically solvable yet over-determined). That is a more operational diagnostic taxonomy than we wrote down.

**Kernel relevance.** Medium. If Keel grows a sketch/assembly solver, adopt per-constraint status flags including a "formally satisfied but contradictory" state. Feeds dossier 04/12.

## Theme 4: C3D Converter (import / healing)

### Source 8: Prokof'eva, "C3D Converter: Plug and Play" (2024); Converter product page

**Citation.** K. Prokof'eva (К. Прокофьева), "C3D Converter: Plug and Play," Habr (ASCON blog), 28 May 2024, https://habr.com/ru/companies/ascon/articles/1040660/ ; product page https://c3dlabs.ru/products/c3d-toolkit/converter/ . (Both fetched.) STEP support AP203/AP214/AP242.

**Content (translated).** This release post is about format breadth and API simplification, not healing internals. The only repair detail: extended handling of meshes carrying topological information, with optional **automatic mesh healing on import (автоматическое лечение сеток при импорте)**. STEP import covers AP203/214/242. No description of shell stitching, gap closure, or tolerant-edge synthesis appears in this post (those mechanisms are described instead in Sources 3 and 5 under tolerant geometry).

**Novelty verdict: (c) redundant.** Our STEP/import dossiers (13, 30, 38) already cover healing and stitching at this level; mesh auto-heal-on-import is a known idea.

**Kernel relevance.** Low for this post specifically; the *real* converter-relevant technique (tolerant edges absorbing import gaps) is captured under Sources 3 and 5.

## Theme 5: ASCON / KOMPAS-3D Lineage

### Source 9: C3D origin and KOMPAS lineage

**Citation.** "Znakom'tes - geometricheskoe yadro C3D" (Знакомьтесь – геометрическое ядро C3D), authors N. Golovanov, O. Zykov, Yu. Kozulin, A. Maksimenko, *SAPR i grafika* (САПР и графика), No. 4, April 2013, https://sapr.ru/article/23756 ; C3D Toolkit Wikipedia, https://en.wikipedia.org/wiki/C3D_Toolkit ; LEDAS/C3D release notes.

**Content (translated).** The introductory article by the kernel's authors confirms the lineage: C3D is the kernel extracted from KOMPAS-3D and commercialized by C3D Labs (founded 2012, ASCON group). The 2013 article is functionality-oriented; on robustness it only mentions handling both explicit degeneracy (e.g. an offset that cannot be built) and implicit degeneracy of surfaces (явное и неявное вырождение поверхностей). It does **not** lay out a tolerance framework, which is why the 2024-2025 posts (Sources 3, 5) are the substantive ones.

**Novelty verdict: (c) redundant / context only.** The lineage is already known to our corpus. The degeneracy handling (явное/неявное вырождение) is the only kernel nugget and overlaps dossiers 28/41 (fillet/feature failure).

**Kernel relevance.** Low-medium. Confirms that "the C3D book = the KOMPAS kernel rationale," which strengthens citing Golovanov as a primary architecture source.

## Theme 6: Soviet / Russian Computational-Geometry and Spline Theory

### Source 10: The Novosibirsk (Siberian) spline school

**Citation.** Yu. S. Zavyalov, B. I. Kvasov, V. L. Miroshnichenko (Ю. С. Завьялов, Б. И. Квасов, В. Л. Мирошниченко), *Metody splain-funktsii* (Методы сплайн-функций, "Methods of Spline Functions"), Moscow: Nauka, 1980; and Yu. S. Zavyalov, V. A. Leus, V. A. Skorospelov (Завьялов, Леус, Скороспелов), *Splainy v inzhenernoi geometrii* (Сплайны в инженерной геометрии, "Splines in Engineering Geometry"), Moscow: Mashinostroenie, 1985. Refs: http://lib.mexmat.ru/books/57910 ; https://urss.ru/cgi-bin/db.pl?page=Book&id=5314 ; https://search.rsl.ru/ru/record/01001238941 .

**Content (translated/summarized; from catalog and secondary descriptions, primary PDFs not fetched).** A distinct and substantial Soviet spline tradition centered in Novosibirsk (Akademgorodok). It developed spline interpolation/approximation, numerical differentiation and integration via splines, and boundary-value solutions, with emphasis on algorithms "efficiently implemented on a computer." *Splines in Engineering Geometry* applies this specifically to engineering surface and curve construction. Kvasov later became known internationally for **shape-preserving and GB-splines (generalized B-splines), and tension/hyperbolic splines** for monotone/convexity-preserving interpolation.

**Novelty verdict: (b), trending toward redundant.** The Western canon (de Boor, Schoenberg, Piegl-Tiller) covers the same B-spline foundation; Kvasov's shape-preserving / tension-spline work *was* published in English and is in the international literature. So this is a parallel tradition rather than hidden knowledge. The one item worth a second look for a kernel is **tension/GB-splines for guaranteed monotone or convex interpolation**, which is occasionally useful for fairing and for clean offset/blend cross-sections, but it is not Parasolid-class-novel.

**Kernel relevance.** Low-medium. Note tension/shape-preserving splines as an option for fairing (dossier 32); otherwise covered.

### Source 11: Descriptive-geometry (начертательная геометрия) and applied-geometry tradition

**Citation.** Russian descriptive-geometry pedagogy on surface intersection, e.g. MPEI course material "Peresechenie poverkhnostei" (Пересечение поверхностей), https://mpei.ru/.../Intersection-of-surfaces.pdf ; nachert.ru course pages; the GraphiCon (ГрафиКон) conference series, https://www.graphicon.ru/ .

**Content (translated).** The Russian descriptive-geometry school formalizes surface-surface intersection through **auxiliary intermediary surfaces (поверхности-посредники)**: choose auxiliary planes or spheres that cut both input surfaces along the simplest possible curves (lines or circles), then intersect those. The sphere-intermediary method (метод сфер) handles surfaces of revolution with intersecting axes elegantly.

**Novelty verdict: (c) redundant as kernel technique.** This is a *manual/draughting* method and a teaching framework, not a numerical kernel SSI algorithm. Production SSI is marching/subdivision/lattice-based (our dossiers 11, 26), and Golovanov's own Ch. 4 uses the numerical approach. The intermediary-surface idea is conceptually the same as choosing good seed/auxiliary curves but offers nothing a kernel does not already do.

**Kernel relevance.** Low. Cultural/foundational context only.

### Source 12: GraphiCon proceedings (modern Russian graphics/geometry venue)

**Citation.** GraphiCon (ГрафиКон) annual proceedings, e.g. GraphiCon 2025 paper on "alpha-beta triangulation" theory in E3, https://www.graphicon.ru/html/2025/papers/paper_092.pdf ; venue index https://www.graphicon.ru/ .

**Content (translated/summarized; abstract-level only).** GraphiCon is the main Russian computer-graphics and geometric-modeling conference. Recent geometric-modeling papers include work defining an "alpha-beta triangulation" and its optimality properties for free-form surfaces in 3D Euclidean space. The venue also hosts realistic-rendering and computational-optics work.

**Novelty verdict: (b)/(c).** Triangulation-quality theory overlaps our tessellation dossier (05). Without fetching full PDFs I cannot claim a specific novel kernel technique from GraphiCon; the abstracts read as incremental refinements of mesh-quality criteria already in the Western literature (Delaunay, Chew, Shewchuk).

**Kernel relevance.** Low on present evidence. Worth a deeper future pass only if a specific tessellation problem needs it; not a robustness or boolean source.

## What the Russian Literature Adds for Keel (Synthesis)

**Honest size of the gap: small, but not zero, and concentrated in exactly the place the brief predicted: tolerant modeling.**

The English corpus was not missing the *existence* of C3D's ideas. Golovanov's book is in English, the booleans/topology/constraint material mirrors Parasolid/ACIS, the Soviet spline school was largely re-published internationally, and the descriptive-geometry tradition is pedagogy, not kernel algorithms. Three of the six themes (Soviet splines, descriptive geometry, GraphiCon, plus the converter and 2018 overview posts) returned **redundant or merely-deeper** material. So the brief's hypothesis that "the gap may be small" is substantially correct, and it should be stated plainly.

The non-zero part is the C3D Labs **engineering blog corpus**, which is Russian-only and which our corpus had never mined for *algorithmic* detail (it only knew C3D as the 500k-model benchmark). From it, the genuinely category-(a) findings worth adopting are:

1. **The explicit tolerant-topology geometric model (Source 3):** tolerance lives on the topological entity, not the geometry. A tolerant vertex *is* a sphere of radius t; a tolerant edge *is* a tube of radius t; the carried curve/surface stays mathematically exact. This is the cleanest specification of tolerant B-rep we have found in any language, and it is directly implementable in Keel's `Vertex`/`Edge` types. (Feeds dossiers 17, 29, 30, 37, 39.)

2. **Per-operation boolean precision as a first-class API argument (Source 5):** the boolean takes its tolerance as a call parameter, not from global kernel state. This is an API-design lesson, not just a value. (Feeds dossier 39.)

3. **Connectivity repair by tolerant-vertex insertion (Source 5):** when wireframe/shell edits would break connectivity, restore it by *inserting a tolerance vertex* rather than perturbing geometry. (Feeds dossier 13.)

4. **"Controlled/managed accuracy" budget per operation, and an explicit inaccuracy budget for import-derived bodies (Source 4).** (Feeds dossier 37.)

5. **Smaller, useful specifics:** the median-shell-as-locus-of-bitangent-spheres construction (Source 3, feeds dossier 10); per-constraint four-status diagnostics including "formally satisfied but contradictory" (Source 7, feeds dossier 04/12); tension/shape-preserving GB-splines for fairing (Source 10, feeds dossier 32).

Everything else, the topology data structures, the boolean engine at a high level, variational constraint solving, STEP healing, surface intersection numerics, is already in the English corpus at equal or greater depth. The single most valuable action item is to absorb the tolerant-topology model from Source 3 into Keel's tolerance design, and to cite Golovanov's (English) *Geometric Modeling* as a primary architecture reference rather than treating C3D as a black-box benchmark.

## References

1. N. N. Golovanov (Голованов Н. Н.), *Geometricheskoe modelirovanie* (Геометрическое моделирование, "Geometric Modeling"), КУРС/ИНФРА-М, 2024, ISBN 978-5-905554-76-6; first ed. Fizmatlit, 2002. TOC: https://znanium.ru/catalog/document?id=439456 ; https://urss.ru/cgi-bin/db.pl?page=Book&id=10999
2. N. Golovanov, *Geometric Modeling: The Mathematics of Shapes* (English translation), 2015, ISBN 978-1497473195. https://c3dlabs.com/company/book/ ; review https://isicad.net/articles.php?article_num=17461
3. A. Tumanin (Туманин А.), "Tendentsii v razvitii geometricheskogo yadra C3D Modeler" (Тенденции в развитии геометрического ядра C3D Modeler, "Trends in the development of the C3D Modeler kernel"), C3D Labs blog, 2024. https://c3dlabs.ru/blog/products/tendentsii-v-razvitii-geometricheskogo-yadra-c3d-modeler/
4. A. Tumanin (Туманин А.), "C3D Modeler - osnova geometricheskogo yadra C3D" (C3D Modeler – основа геометрического ядра C3D, "C3D Modeler, the basis of the C3D kernel"), Habr, 2023. https://habr.com/ru/companies/ascon/articles/762206/
5. T. Kondrikova (Кондрикова Т.), "Geometricheskoe yadro C3D: novye funktsii i napravleniya razvitiya" (Геометрическое ядро C3D: новые функции и направления развития, "C3D kernel: new functions and development directions"), Habr, 2025. https://habr.com/ru/companies/ascon/articles/967270/
6. C3D Labs, "Obzor SAPR na geometricheskom yadre C3D" (Обзор САПР на геометрическом ядре C3D, "Overview of CAD systems on the C3D kernel"), Habr, 2018. https://habr.com/ru/companies/ascon/articles/431918/
7. A. Alakverdyants (Алаквердянц А.), "C3D Solver: printsipy parametricheskogo chercheniya 2D-patternov..." (C3D Solver: принципы параметрического черчения 2D-паттернов..., "C3D Solver: principles of parametric 2D patterns and 3D assembly improvements"), Habr, 2024. https://habr.com/ru/companies/ascon/articles/788338/ ; product page https://c3dlabs.ru/products/c3d-toolkit/solver/
8. K. Prokof'eva (Прокофьева К.), "C3D Converter: Plug and Play," Habr, 2024. https://habr.com/ru/companies/ascon/articles/1040660/ ; product page https://c3dlabs.ru/products/c3d-toolkit/converter/
9. N. Golovanov, O. Zykov, Yu. Kozulin, A. Maksimenko, "Znakom'tes - geometricheskoe yadro C3D" (Знакомьтесь – геометрическое ядро C3D, "Meet the C3D geometric kernel"), *SAPR i grafika* (САПР и графика) No. 4, 2013. https://sapr.ru/article/23756 ; C3D Toolkit: https://en.wikipedia.org/wiki/C3D_Toolkit
10. Yu. S. Zavyalov, B. I. Kvasov, V. L. Miroshnichenko (Завьялов, Квасов, Мирошниченко), *Metody splain-funktsii* (Методы сплайн-функций, "Methods of Spline Functions"), Nauka, 1980; Yu. S. Zavyalov, V. A. Leus, V. A. Skorospelov, *Splainy v inzhenernoi geometrii* (Сплайны в инженерной геометрии, "Splines in Engineering Geometry"), Mashinostroenie, 1985. http://lib.mexmat.ru/books/57910 ; https://search.rsl.ru/ru/record/01001238941
11. Russian descriptive-geometry (начертательная геометрия) intersection method (поверхности-посредники, метод сфер), MPEI/nachert.ru course materials. https://mpei.ru/ (Intersection-of-surfaces.pdf); https://nachert.ru/
12. GraphiCon (ГрафиКон) proceedings, geometric-modeling track, 2025. https://www.graphicon.ru/

### Method notes and limitations
- Golovanov's full book text was not fetched; book content is from the published TOC plus the C3D Labs blog corpus by the same authors. The book exists in English (ref. 2).
- Sources 3, 4, 6, 7, 8 were fetched as primary Russian pages. Source 5 (2025 article) was partially obtained via a search-engine summary after one fetch and one connection-refused refetch; its API-argument and tolerant-vertex-insertion claims should be re-verified against the live page before being treated as load-bearing.
- The fizmatlit TOC mirror (fizmatlit.narod.ru/golovan.htm) was unreachable (connection refused); the TOC used is from the znanium.ru publisher catalog.
- Sources 10, 11, 12 are at catalog/abstract level; their full PDFs were not fetched, so no specific novel kernel algorithm is claimed from them.
