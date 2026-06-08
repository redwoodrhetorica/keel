# 43. Chinese-Language CAGD and CAD-Kernel Literature: A Native-Language Pass

## Scope

This dossier is a Chinese-language literature pass on CAD geometry-kernel and computer-aided geometric design (CAGD) research, conducted to surface knowledge that exists in Chinese-language sources but is absent or thin in the English-language corpus that built dossiers 01-42. The English corpus (roughly 40 dossiers, ~1030 cited sources) was assembled entirely from English-language publications. Two facts motivated a native-language pass:

1. The leading Chinese CAGD researchers (Falai Chen at USTC on mu-bases and implicitization, Hongwei Lin at Zhejiang on progressive-iterative approximation, the USTC/Zhejiang surface-intersection line) publish their primary results in English, but there is additional Chinese-only journal output, especially algorithmic refinements and surveys in 计算机辅助设计与图形学学报 (Journal of Computer-Aided Design and Computer Graphics, "JCAD").

2. Since roughly 2018-2020 China has invested heavily in DOMESTIC CAD KERNELS as a 卡脖子 (bottleneck / "stranglehold") technology that industrial-software self-sufficiency depends on. The engineering and industry literature on those kernels (中望/ZWSOFT Overdrive, 山大华天/Huatian CRUX, 科大九韶/AMCAX, and others) is almost entirely Chinese-language and was completely absent from the English corpus. This is the highest-value, least-covered area, and it is where this pass concentrates.

## Method and the English-Corpus Baseline

Searches were run in simplified Chinese with WebSearch; Chinese pages and bilingual journal PDFs were fetched with WebFetch and, where the fetch returned a binary PDF, extracted locally with `pdftotext`. The Chinese glyphs do not survive `pdftotext` font de-embedding on JCAD's PDFs, but the bilingual English abstracts (JCAD prints a full English abstract on every paper) extract cleanly, so the abstract-level findings here are from the authors' own English abstracts, not machine translation. Where content comes from a Chinese web page (company sites, CSDN/Zhihu/163 reporting, USTC and AMSS pages), I translated key passages and quote the Chinese term of art in parentheses. Several primary Chinese platforms (CSDN, Zhihu, jishulink) hard-block automated fetching with HTTP 403/521; for those, content is drawn from the search-engine summary of the page plus corroborating accessible sources, and that provenance is flagged per entry.

Baseline check against the existing corpus (grep over dossiers 01-42): the English corpus does NOT name the Yang/Jia/Yan topology-guaranteed SSI line, Hongwei Lin's PIA/LSPIA family, or Falai Chen's mu-basis lineage as tracked topics, and contains nothing on any domestic Chinese kernel. It DOES already cover, at a deep level, the underlying problems those lines address: surface-surface intersection and loop/tangency handling (files 11, 39, 40), boolean robustness and the geometry-topology consistency view of robustness (files 29, 30, 39), tolerant modeling and epsilon-solidity (files 29, 30), fillet/blend generation including the spine-as-SSI canal-surface model and radius-vs-feature failure (files 28, 40, 41), n-sided and constrained surfacing (files 26, 32, 33), and canonical/analytic recovery (file 24). So the novelty test throughout is: does the Chinese source add a *mechanism or engineering account* the corpus lacks, or only a *named instance* of a problem the corpus already solved?

A note on honesty: the domestic-kernel area is marketing-heavy. Company and trade-press sources mix verifiable engineering detail with promotion ("真自主", "更硬核", "破壁之路"). Each entry separates the two and labels promotional claims as such.

---

## Theme 1: The Domestic-Kernel Engineering Effort (highest-value, least-covered)

The single most important structural finding of this pass: the three most-cited Chinese 3D kernels split cleanly into TWO acquired-and-domesticated kernels and ONE from-scratch kernel, and that distinction is the real story, not the marketing.

- **中望 Overdrive** (ZWSOFT) and **华天 CRUX IV** (Shanda Huatian) are both *acquired* Western/Japanese kernels with source code obtained and then developed in-house. They are "自主可控" (autonomously controllable) in the IP/source-ownership sense, not "from-scratch."
- **科大九韶 AMCAX** (USTC spin-off Jiushao) is the one publicly described *from-zero* ("从0到1完全自主研发") kernel, and its development order and module list are engineering-legible in a way the others' are not.

### 1.1 中望 Overdrive (ZWSOFT)

**Citation.** ZWSOFT product and news pages plus trade-press reporting: "天工开物: 中望CAD全栈技术架构深度解构" (CSDN, 2024/2025, fetch-blocked, used via search summary); "中望overdrive几何内核合作交流会在沪举办" (NetEase/163 reprint, 2024); "中国CAD稳了!中望软件2026年度产品发布会" (2026); ZWSOFT Overdrive product page (zwsoft.cn/product/overdrive).

**Content (translated).** Overdrive originates from the geometry engine of the American **VX Corporation**, a kernel with roughly 25 years of history; ZWSOFT acquired VX in 2010 and obtained the full source code and IP, which is the basis for the "真自主" (truly autonomous) claim, meaning it cannot be 卡脖子 (cut off) by a third-party licensor. Overdrive reached "full commercialization" (全面商业化) in its 2026 product cycle. The publicly emphasized engineering work is concentrated in fillet/blend, booleans, and complex-model performance: "智能圆角处理" (intelligent fillet handling), "自动圆角" (automatic filleting) supporting *different radii on concave vs convex edges*, "非对称面圆角" (asymmetric face fillet), and "Conic面圆角" (conic-face fillet), targeting "多交汇、深腔狭缝" (multi-junction and deep narrow-cavity) cases while "减少圆角面数量、保证边线完整性" (reducing the number of fillet faces and preserving edge-line integrity). They report "G2/G3 高阶连续" (G2/G3 higher-order continuity) for automotive/marine/aerospace, improved hole-fill ("补洞算法") continuity, "Cage Modeling" enhancements, and "自交去除" (self-intersection removal) technology giving "源头级" (source-level) guarantees of solid validity. Performance claims: average >30% speedup on complex models, >20% API response improvement. One revealing engineering phrasing from the cooperation-meeting report: fillet/blend is called the "珠穆朗玛峰" (Mount Everest) of the geometry kernel.

**Novelty verdict.** (b) Chinese-language *engineering account* of problems already in the corpus, plus (c) marketing. The fillet feature list (concave/convex differential radius, asymmetric, conic-face, multi-junction, deep-cavity, fewer-faces-with-edge-integrity) is exactly the capability surface dossiers 28/40/41 specify; the "self-intersection removal for source-level solid validity" restates the epsilon-solidity / valid-topology-is-non-negotiable rule of file 29. Nothing here is a new mechanism. Its value is corroborative: an independent commercial kernel team, working in the same problem space, *independently ranks fillet/blend as the hardest single subsystem* ("Mount Everest"), which confirms the corpus's decision to spend three dossiers (28, 40, 41) on it. The VX-acquisition provenance is genuinely new factual context (the corpus did not record it) but not technically actionable for Keel.

**Kernel relevance.** Confirmatory prioritization signal for the blend engine; no new algorithm. The "different radii on concave vs convex edges" framing is a concrete UX/spec detail worth mirroring in Keel's fillet API.

### 1.2 华天 CRUX IV (山大华天 / Shanda Huatian, SINOVATION)

**Citation.** "华天软件 SINOVATION 9.1 自主可控三维CAD内核CRUX IV 历史由来" (jishulink 技术邻 post 1816853, fetched; corroborated by CSDN/Zhihu reprints of the same article).

**Content (translated).** CRUX IV derives from the geometry engine of **Japan's UEL Corporation** (its CADmeister software), originally developed for Toyota's manufacturing needs with 20+ years of automotive-industry accumulation. In May 2008 UEL signed a technology-transfer agreement opening its core technology to Huatian and 中创软件 (Censoft); Huatian obtained the source. The kernel implements B-rep topology with "混合建模" (mixed/hybrid modeling of wireframe + surface + solid), "参数化设计" (parametric design), and a geometric constraint solver ("几何约束求解器"). Kernel development was led by chief scientist Dr. 梅敬成 (Mei Jingcheng), formerly of France's think3. The article positions Huatian and ZWSOFT as the rare Chinese vendors holding full source for both a geometry kernel and a constraint solver.

**Novelty verdict.** (c) redundant technically; (new only as factual provenance). The B-rep + hybrid-modeling + parametric + constraint-solver stack is the standard kernel architecture the corpus already describes. No mechanism, no robustness technique, no data-structure detail beyond "B-rep" is disclosed. The single useful fact is provenance: CRUX IV is a domesticated CADmeister/UEL kernel, which (like Overdrive-from-VX) means the publicly visible Chinese 3D-kernel landscape is substantially *acquired* Western/Japanese technology, not independent reimplementation. This is an important honest correction to any assumption that "domestic kernel" implies "independently built."

**Kernel relevance.** None algorithmically. Useful only as competitive/landscape context.

### 1.3 科大九韶 AMCAX (Jiushao, USTC spin-off): the one from-scratch account

**Citation.** AMCAX official documentation (docs.amcax.net/v4_7_0, fetched); AMCAX product site (product.amcax.net, amcax.net); "新一代工业软件内核发布" (Anhui Center for Applied Mathematics / ACAM USTC, 2022, fetched); Zhihu feature "国产完全自主可控三维建模引擎: 九韶内核AMCAX" (p/632573148, via search summary); roadmap data from multiple corroborating Chinese pages.

**Content (translated).** AMCAX is described as a "从0到1完全自主研发" (from-zero, fully independently developed) CAD/CAE/CAM kernel "源自中国科学技术大学几代人四十多年的学术积累" (drawing on 40+ years of academic accumulation across generations at USTC). The team (科大九韶, formed 2017) is led by academician **鄂维南 / E Weinan** with **杨周旺 / Yang Zhouwang** (executive director of the Anhui Applied Mathematics Center) as chief scientist. The publicly stated release order is engineering-legible and unusual:

- **AMCAX 1.0** (Jan 2021): polygon/mesh modeling first ("多边形网格建模").
- **AMCAX 2.0** (Sep 2022): "原创突破" T-mesh spline modeling ("T网格样条建模"), the differentiating module.
- **AMCAX 3.0** (Jun 2023): parametric feature modeling + geometric constraint solver.
- **AMCAX 4.0** (Sep 2024): NURBS surface modeling.
- **AMCAX 5.0** (Sep 2025): latest release.

The official docs decompose the kernel into: **Common** (points, vectors, bounding boxes); **Part** = Topology & Geometry (B-rep boundary representation), NURBS & Modeling, Boolean (union/intersection/difference), Fillet/Offset, Healing & HLR; plus **GCS** (geometric constraint solver, distance/angle constraints); **SubD** (subdivision surfaces from polygon meshes) and **TMSpline** (T-mesh splines) as distinct modules; **Step/Iges/OCCTIO** translators (STEP AP203/214/242, IGES, OCC BREP); **Meshing** (global/local sizing, growth rates); and **GeomE** (geometry cleaning/repair). Stated build constraints: ISO C++17; the Meshing module depends on **GMP 6.2.1, MPFR 4.2.0, and TBB 2021.10.0**. Reported deployments span furniture, architecture, mining, mechanical equipment, and semiconductor metrology.

**Novelty verdict.** (a) genuinely new as an *engineering account*, the most valuable single find of this pass, with the caveat that it is a public module list and roadmap, not internal algorithm disclosure. What is new and useful:

1. **The development order is itself a thesis.** AMCAX built mesh → T-mesh splines → parametric features/constraints → NURBS, i.e. it treated **T-spline / T-mesh modeling as the foundational original layer and added classical NURBS B-rep on top later.** This inverts the usual kernel order (NURBS B-rep first). It is the concrete realization of the USTC CAGD lineage (Falai Chen mu-basis, isogeometric/T-spline emphasis) as a shipping kernel, and it is a live data point for Keel's own representation-priority debate. The English corpus discusses T-splines and isogeometric analysis (files 18, 31-33 touch T-splines and U-splines) but never as a *kernel's primary modeling substrate*; AMCAX is an existence proof that a serious kernel team chose that path.
2. **The dependency set (GMP + MPFR + TBB) is corroborative.** It confirms, from an independent from-scratch kernel, the exact numerical-substrate choices the corpus recommends: exact/extended-precision arithmetic (GMP rationals, MPFR multiprecision floats) under a task-parallel runtime (TBB). This matches file 37 (numerical substrate) and file 36 (parallelism) directly. An independent team converging on GMP/MPFR/TBB strengthens those recommendations.
3. **SubD and TMSpline as first-class peer modules** to B-rep is an architectural choice worth noting: AMCAX does not treat subdivision/T-spline as a bolt-on but as co-equal representations, consistent with the corpus's "go non-manifold and multi-representation from day one" stance (file 02).

What is NOT disclosed and therefore not new: the topology data structure (only "B-rep" is named, no radial-edge/partial-entity detail), the boolean robustness method, the tolerant-modeling approach, the SSI algorithm. So AMCAX validates corpus-level architecture decisions but offers no extractable algorithm.

**Kernel relevance.** High as a *strategic/architecture* reference, low as an algorithm source. Three actionable takeaways: (i) AMCAX is a direct existence proof that GMP/MPFR/TBB is a viable from-scratch kernel substrate (de-risks file 37's recommendation); (ii) its T-mesh-first order is a serious alternative worth a deliberate decision in Keel, not an accident; (iii) AMCAX is the natural "comparable open-ish reference kernel" to track going forward, more so than the acquired Overdrive/CRUX.

### 1.4 The domestic-kernel gap (industry analysis)

**Citation.** "工业软件行业专题报告: 工业软件底层技术剖析" (Tencent News / qq.com finance report, 2021); "国内的CAD企业是否都依赖第三方几何内核" (Zhihu Q&A 445615319, fetch-blocked HTTP 403, via search summary); 东方财富 ZWSOFT equity research PDF (dfcfw.com, 2022); kernel-comparison pages (innovation4.cn, caxkernel.com 卡核).

**Content (translated).** Consensus across these sources: Siemens **Parasolid** and Dassault/Spatial **ACIS** are the "两大内核阵营" (two great kernel camps), both built around 1985 and adopted widely by CAD vendors in the 1990s; Parasolid is generally rated "最成熟、应用最广" (most mature, most widely applied) and "Parasolid要优于ACIS" in CAD functionality. The structural problem ("卡脖子"): most Chinese CAD vendors build on a *third-party* kernel (ACIS, Parasolid, or OpenCASCADE) and are therefore exposed to being cut off. OpenCASCADE is judged usable for small projects but "无论从功能、稳定性、性能上，都难以用于大规模的实际商用" (difficult to use for large-scale real commercial deployment in function, stability, or performance), which is the same all-or-nothing-robustness critique file 29 levels at OCCT. The reports' verdict: domestic kernels have progressed but "与Parasolid和ACIS相比仍存在差距" (still lag Parasolid and ACIS), and the gap is largest in robustness on complex/dirty industrial models and in the breadth of validated advanced operations (blends, complex booleans).

**Novelty verdict.** (b)/(c) mostly. The OCCT critique and the "robustness on dirty industrial data is the moat" diagnosis are exactly file 29's central thesis, here independently restated by Chinese industry analysts. New only as external corroboration: the Chinese industry's own framing locates the gap precisely where the corpus locates Parasolid's differentiation (robust booleans + blends + tolerant modeling on imperfect real-world bodies, validated against a huge regression corpus). This is a strong independent confirmation of the file-29 "robustness is an organizational/corpus asset" conclusion.

**Kernel relevance.** Strategic confirmation that Keel's hardest-and-most-differentiating investment is the same one the Chinese national effort is struggling with: robust booleans/blends on dirty geometry, backed by a large regression corpus.

---

## Theme 2: Surface-Surface Intersection and Boolean Robustness

### 2.1 Efficient B-spline curve/curve intersection (Wang, Lyu, Chen Xiaodiao)

**Citation.** Wang Yongao (王永傲), Lyu Hangting (吕航挺), Chen Xiaodiao (陈小雕). "An Efficient Method for Computing the Intersection between Two B-Spline Curves" (一种高效的两条B样条曲线求交方法). Journal of Computer-Aided Design & Computer Graphics, Vol.36 No.5, May 2024, pp. 688ff. DOI 10.3724/SP.J.1089.2024.19869. Hangzhou Dianzi University. Fetched as bilingual PDF; abstract is the authors' own English.

**Content (translated, from the English abstract and extracted body).** A hybrid method for intersecting two B-spline curves with three contributions: (1) a **linear-complexity clipping method** to obtain good initial values, contrasted against the standard quadratic O(n^2) bounding behavior; (2) a **derivative-free verification of traversal cases** generalizing the secant method, with a higher efficiency index (the paper cites convergence orders 1.414, 1.618, 1.839 for the family vs Newton's quadratic 2); (3) a **new iteration of convergence rate 2 for the contact (tangent) case**, the case where prevailing Newton and clipping methods degrade badly. Reported: ~10% speedup over Newton for transversal intersections and **100%-300% speedup for contact/tangent intersections.** The authors note it can be combined with root isolation to handle non-polynomial curves.

**Novelty verdict.** (b) Chinese-language extension of a corpus-covered problem, with a genuinely useful nugget. The corpus (files 11, 39, 40) already treats SSI/curve intersection, clipping vs marching, and explicitly flags the *tangent/contact* case as the hard one. This paper does not change the landscape, but it offers a concrete, recent (2024) *quadratically convergent iteration specifically for the tangent intersection case* with measured 2x-4x speedups. That specific tangent-case iteration is the kind of detail the corpus gestures at ("tangency is the hard case") but does not give a formula for. Worth extracting at the algorithm level.

**Kernel relevance.** Medium-high and concrete: Keel's intersection code must handle tangent/contact intersections (file 39's coincident/tangent face booleans depend on exactly this), and a derivative-free, order-2-on-tangency iteration with published speedups is directly evaluable. This is the single most directly-adoptable algorithm found in the pass.

### 2.2 Falai Chen's implicitization / moving-surface / mu-basis lineage and SSI

**Citation.** Falai Chen (陈发来), USTC faculty research pages (faculty.ustc.edu.cn/chenfalai; math.ustc.edu.cn; acam.ustc.edu.cn). Foundational works (published in English, indexed here for the Chinese-language follow-up context): Chen and Sederberg, moving-surface method (1995); Chen, Cox, Sederberg, mu-basis of a rational curve (1998); extensions to ruled surfaces, moving planes/quadrics for surfaces with base points, and "topologically correct intersection curves of trimmed quadrics with tolerance control" (joint work with Xiaoshan Gao 高小山 on efficient/reliable surface-intersection computation).

**Content (translated).** Chen's program builds a "桥梁" (bridge) between parametric and implicit representations. The **moving-surface method** (动曲面方法) expresses a surface's implicit equation as a *low-order determinant of moving planes/quadrics* and, unlike resultants/Gröbner/Wu-elimination, "remains effective for surfaces with base points" (对有基点的曲面仍然有效). The **mu-basis** (μ-基) gives a compact representation carrying both forms simultaneously. Chen's SSI work emphasizes **topologically correct** intersection curves with explicit **tolerance control** (拓扑正确 + 容差控制), and his algebraic-surface modeling uses piecewise low-degree algebraic patches for transition surfaces and hole-filling.

**Novelty verdict.** (a) as a tracked line the corpus was missing, but (b) at the mechanism level because the primary results are in English and an English-corpus reader could in principle have found them. The corpus never named the mu-basis / moving-surface lineage despite covering implicitization-adjacent topics (file 11 mentions implicitization in passing for certified meshing). What is genuinely worth importing: the **moving-surface implicitization that survives base points** is a robustness property the corpus's implicitization discussion does not capture, and it bears directly on robust intersection of rational surfaces (a base point is exactly where naive implicitization fails). The "topologically correct trimmed-quadric SSI with tolerance control" line is the Chinese analogue of the corpus's topology-guaranteed SSI concern (files 11, 39).

**Kernel relevance.** Medium. Keel is unlikely to make implicitization a primary path (the corpus's verdict favors parametric marching + exact predicates), but mu-basis/moving-plane implicitization is the best-available exact tool for *rational* surface intersection and base-point-robust point classification, and the lineage is now correctly identified for deeper follow-up if Keel ever needs algebraic SSI.

### 2.3 Curve/surface intersection survey (Chinese-and-international synthesis)

**Citation.** "几何算法: 曲线曲面求交的方法总结(国内外文献调研)" (luolei188, CSDN, 2023, fetch-blocked HTTP 521, via search summary); corroborated by "曲面求交概况" (CSDN he626shidizai). Secondary, not a primary peer-reviewed source; flagged as such.

**Content (translated).** Classifies SSI into subdivision/clipping (裁剪/分割), tracing/marching (追踪), algebraic/implicitization (代数法), and hybrid (subdivision-to-seed then marching, 分割→追踪). Notes the NURBS convex-hull / control-mesh ray-count bound used to guarantee no intersection is missed (adaptive subdivision until at most one intersection per region). Mentions an osculating-plane (密切平面) tracing scheme that reduces a 3D tracing problem to a more tractable local one. States the robustness problem of solid modeling is fundamentally a **geometry-topology inconsistency** ("几何-拓扑不一致问题"): SSI error causes membership-classification (成员判别) errors in booleans.

**Novelty verdict.** (c) redundant. Every classification and the geometry-topology-consistency framing is already in the corpus (files 11, 29, 39). Included only to record that the Chinese practitioner literature converges on the identical taxonomy and the identical root-cause diagnosis. The osculating-plane tracing detail is a minor known technique, not new.

**Kernel relevance.** None new; confirms taxonomy alignment.

---

## Theme 3: Fitting and Approximation, the PIA / LSPIA Family (Hongwei Lin and successors)

This is the most ACTIVE Chinese-only output stream found: a continuous flow of LSPIA refinements in JCAD and the Zhejiang University journal, almost none of which appears in the English corpus.

### 3.1 The foundational review

**Citation.** Lin Hongwei (林宏伟/林宏伟). "几何迭代法及其应用综述" (A Survey of Geometric Iteration Methods and Their Applications). Journal of Computer-Aided Design & Computer Graphics, Vol.27 No.4, 2015. (Identified via search; the survey is the anchor of the line.)

**Content (translated).** Establishes "几何迭代法" (geometric iteration / progressive-iterative approximation, PIA) as a family: starting from an initial fit, iteratively adjust control points by feeding back the fitting error, with the limit converging to interpolation (PIA) or to the least-squares fit (LSPIA). The appeal is that each step is a simple, geometrically meaningful, embarrassingly local update that avoids assembling and solving a global linear system.

**Novelty verdict.** (a) as a tracked topic the corpus omitted. The English corpus's fitting/metrology dossier (file 23) and surfacing dossiers (32, 33) solve fitting via constrained least squares / KKT systems and never mention PIA/LSPIA. PIA is a genuinely different fitting *paradigm* (iterative control-point feedback vs one global solve) with a real engineering tradeoff: no large linear solve, trivially parallel and incremental, at the cost of iteration count. That tradeoff is new information for Keel's fitting subsystem.

**Kernel relevance.** Medium. For interactive/streaming fitting and for very large point sets where a global solve is expensive, LSPIA is a real alternative to the corpus's KKT-least-squares approach (file 33). Worth tracking as an option, not a replacement.

### 3.2 GS-LSPIA (Gauss-Seidel acceleration)

**Citation.** Hamza Yusuf Fatihu, Jiang Yini (蒋艺霓), Lin Hongwei (林宏伟). "Gauss-Seidel最小二乘渐进迭代逼近" (Gauss-Seidel Least-Squares Progressive Iterative Approximation). JCAD, DOI 10.3724/SP.J.1089.2021.18289. Zhejiang University. Fetched.

**Content (translated).** Classical LSPIA converges slowly; GS-LSPIA replaces the Jacobi-style simultaneous control-point update with a **Gauss-Seidel** sweep (use already-updated neighbors within the same iteration). Result: same accuracy in fewer steps and shorter runtime, with the limit still the least-squares solution.

**Novelty verdict.** (b) extension. A standard numerical-analysis acceleration (Jacobi → Gauss-Seidel) applied to a fitting iteration the corpus does not track. Incremental.

**Kernel relevance.** Low-medium; only relevant if Keel adopts LSPIA at all, in which case GS ordering is a free speedup.

### 3.3 CG-CLSPIA (conjugate-gradient constrained LSPIA)

**Citation.** Yang Jinbiao (杨金标), Sun Mengchen (孙梦晨), Hu Qianqian (胡倩倩). "基于共轭梯度的约束最小二乘渐进迭代逼近算法" (Conjugate-Gradient-Based Constrained LSPIA). JCAD, DOI 10.3724/SP.J.1089.2025-00105. Fetched.

**Content (translated).** Constrained LSPIA (CLSPIA) solves "interpolate some points exactly while approximating the rest" but converges slowly. CG-CLSPIA wraps a conjugate-gradient inner iteration (CG-LSPIA solving the unconstrained subproblem via an Uzawa scheme) inside a Lagrange-multiplier outer iteration for the constraints, with a convergence proof. Reported on cubic B-spline curves/surfaces: **average 83.07% fewer total iterations and 55.45% less CPU time** vs CLSPIA.

**Novelty verdict.** (b) extension, but the *constrained* variant is the interesting one for Keel. "Interpolate a subset exactly, approximate the rest, under constraints" is precisely the surfacing problem of files 32/33 (match fixed neighbor surfaces exactly = hard interpolation constraints; fair the rest = approximation). CG-CLSPIA is an iterative alternative to file 33's KKT-solved constrained least squares for that exact problem.

**Kernel relevance.** Medium. Direct alternative formulation for the constrained-surfacing problem the corpus solves via KKT. If Keel's surfacing ever needs an iterative/incremental constrained fitter (e.g., interactive surface matching), this is the relevant Chinese line.

### 3.4 Triangular B-B surface LSPIA acceleration (Schulz / generalized-inverse)

**Citation.** "三角B-B曲面最小二乘渐进迭代格式的革新与加速" (Innovation and Acceleration of the LSPIA Scheme for Triangular Bernstein-Bezier Surfaces). JCAD, DOI 10.3724/SP.J.1089.2022.19010. (Via search; abstract-level.)

**Content (translated).** Extends LSPIA to triangular Bernstein-Bezier (B-B) surfaces and accelerates it using a Schulz-iteration approximation of the Moore-Penrose generalized inverse.

**Novelty verdict.** (b) extension. Notable mainly because it pushes LSPIA onto **triangular** patches, relevant if Keel uses triangular Bezier patches anywhere (e.g., n-sided/transfinite fill, file 26).

**Kernel relevance.** Low-medium; conditional on triangular-patch use.

---

## Theme 4: Implicitization, Isogeometric Analysis, T-Splines, and Implicit Surfaces

### 4.1 Local-refinement splines for isogeometric analysis (Kang, AMSS)

**Citation.** Kang Hongmei (康红梅), "面向等几何分析的局部加细样条" (Local-Refinement Splines for Isogeometric Analysis), Academy of Mathematics and Systems Science (AMSS), CAS, 2025 talk page; plus T-spline variable-mesh isogeometric thin-plate dynamics work (Chinese Journal of Theoretical and Applied Mechanics, 力学学报, 2021, DOI 10.6052/0459-1879-21-199); and a PhD thesis on domain parameterization for IGA.

**Content (translated).** Chinese IGA work centers on **local-refinement spline spaces** (局部加细样条): T-splines, LR-splines, and hierarchical splines that allow refinement in a local region of the T-mesh without propagating across the whole tensor-product grid, via T-mesh node insertion and blending-function refinement. A parallel theme is **domain parameterization** for IGA (building an analysis-suitable spline parameterization of a CAD domain), split into isotropic and r-adaptive (anisotropic) parameterization for problems with local features.

**Novelty verdict.** (b) extension. The corpus touches T-splines and U-splines (files 18, 31) but mainly through the patent/IP lens and as a surfacing representation, not as the *local-refinement + analysis-suitable parameterization* problem that the Chinese IGA community works on. This is a coherent body of work the corpus under-represents, and it is the mathematical substrate underneath AMCAX's T-mesh-first kernel (Theme 1.3). New as a *connected research line to track*, not as a drop-in algorithm.

**Kernel relevance.** Medium and increasingly relevant given AMCAX's choice. If Keel ever pursues isogeometric-ready geometry or T-mesh local refinement, this is the source community. Local-refinement-without-global-propagation is a real capability classical NURBS lacks.

### 4.2 Triply-periodic minimal surfaces (TPMS) survey

**Citation.** TPMS geometric-modeling survey, JCAD Vol.35 No.3, 2023, DOI 10.3724/SP.J.1089.2023.19359 (fetched; English abstract). "Triply Periodic Minimal Surface ... algebraic surface expressed by implicit function."

**Content (translated).** Surveys TPMS (implicit algebraic surfaces, e.g. gyroid/Schwarz) for additive-manufacturing lattice design: mathematical expressions, properties, applications (mechanical, heat/mass transfer, tissue engineering, acoustics), and four modeling-method classes (regular-unit, parametric-unit, region-splicing, global-optimization).

**Novelty verdict.** (c) largely redundant for a B-rep kernel. The corpus covers implicit/mesh/hybrid representation and lattices (files 09, and the convergent-modeling discussion). TPMS is an implicit-modeling/AM-lattice topic adjacent to but outside Keel's exact-B-rep core. Recorded for completeness; not a gap.

**Kernel relevance.** Low. Only relevant if Keel adds implicit lattice modeling.

### 4.3 3D representation and conversion survey (B-rep / mesh / SDF / NeRF)

**Citation.** 3D-representation-and-conversion survey, JCAD Vol.37 No.10, 2025, DOI 10.3724/SP.J.1089.2025-00141 (fetched; English abstract).

**Content (translated).** Reviews point cloud, voxel, mesh, B-rep, and implicit (SDF, NeRF) representations and the conversions among them (point↔mesh, point↔B-rep, voxel↔mesh, mesh↔B-rep, mesh↔implicit), via both classical geometry algorithms and deep learning. Flags the key open challenges as **topology-consistency preservation** (拓扑一致性) and efficiency at high resolution, and points to multimodal fusion and AI-driven conversion as future directions.

**Novelty verdict.** (c) redundant as content (the representations and the mesh↔B-rep reconstruction problem are covered in files 09, and reconstruction work in the surfacing dossiers), but a useful 2025 snapshot of how the Chinese community frames the mesh-to-B-rep conversion problem (the AMCAX 1.0→4.0 pipeline). The honest finding is that even this recent survey names topology-consistency as THE hard problem, the same conclusion the corpus reached.

**Kernel relevance.** Low-medium; confirms mesh→B-rep topology consistency as the recurring hard problem.

### 4.4 Powell-Sabin surface reconstruction (Dalian, with mesh→B-spline)

**Citation.** Yang Zhifei (杨志飞), Shi Xiquan (石熙泉), Wang Weiming (王伟明), Liu Xiuping (刘秀平). "A Parameterized Surface Reconstruction Method Based on Powell-Sabin Subdivision." JCAD Vol.35 No.12, 2023, DOI 10.3724/SP.J.1089.2023.2023-00021. Dalian University of Technology + Delaware State. Fetched; English abstract.

**Content (translated).** Reconstructs a parametric B-spline surface from a triangular mesh that has a coarse quad-partition structure: mean-value parameterization maps each coarse quad to a parameter domain, one Powell-Sabin subdivision refines the triangulation, a bivariate-spline interpolant samples surface points, and a smoothness-energy functional solves the bicubic B-spline control grid. Reports 38% lower vertex-distance MSE vs adaptive algorithms on complex models (e.g. human-head freeform).

**Novelty verdict.** (b) extension. Mesh-to-B-spline reconstruction is in the corpus (surfacing/reconstruction), but the **Powell-Sabin-subdivision-based** parameterization-and-fit pipeline is a specific technique the corpus does not carry. Modest, well-defined contribution.

**Kernel relevance.** Low-medium; relevant only if Keel does scan/mesh-to-NURBS reconstruction.

---

## Theme 5: The Journals (coverage note)

The Chinese-language CAGD/kernel research that matters for Keel concentrates overwhelmingly in **计算机辅助设计与图形学学报 / Journal of Computer-Aided Design & Computer Graphics (JCAD)** (jcad.cn), which is bilingual (full English abstracts) and the de-facto home of the LSPIA line, the intersection-algorithm work, and the representation surveys covered above. Secondary venues: **软件学报 / Journal of Software** (jos.org.cn, broad CS, occasional geometry), **计算机学报 / Chinese Journal of Computers** (cjc.ict.ac.cn), **图学学报 / Journal of Graphics**, **力学学报 / Chinese Journal of Theoretical and Applied Mechanics** (IGA mechanics applications), and **中国科学:信息科学 / Science China Information Sciences** (English-mirrored). Practical finding for future passes: JCAD is the one venue worth monitoring directly; its English abstracts make a native-language pass tractable, and a large fraction of the truly kernel-relevant Chinese work is there. The remaining unique Chinese-language content is the *company/industry* layer (Theme 1), which lives on company sites, CSDN/Zhihu/163/trade-press, not in journals.

---

## What the Chinese Literature Adds for Keel: Honest Synthesis

**Size and nature of the gap: smaller than feared, and concentrated.** The Chinese *academic* literature on the core kernel problems (SSI, boolean robustness, tolerant modeling, fitting) is overwhelmingly either (b) Chinese-language extensions of work whose primary English version the corpus could already reach, or (c) the same taxonomy and the same geometry-topology-consistency root-cause diagnosis the corpus already reached independently. The leading researchers publish their load-bearing results in English. There is no hidden Chinese-only breakthrough on robust/tolerant booleans: a targeted search for a recent self-intersection-removal / robust-boolean Chinese paper returned nothing beyond what the corpus has. That is a genuine negative finding, stated plainly.

**Where there IS genuinely new value (category (a) or strong (b)), in priority order:**

1. **The domestic-kernel engineering accounts, as strategy not algorithm.** The single most valuable new material. The honest structural finding: China's two most-promoted 3D kernels, **Overdrive (from US VX, 2010)** and **CRUX IV (from Japan's UEL/CADmeister, 2008)**, are *acquired-and-domesticated*, not built from scratch; only **AMCAX/九韶 (USTC, from zero, 2017-2025)** is a publicly described independent kernel. AMCAX's public module list and roadmap give three actionable signals: (i) it independently chose **GMP + MPFR + TBB**, corroborating file 37's numerical substrate and file 36's parallelism recommendations from a from-scratch peer; (ii) it built **T-mesh splines as the foundational original layer with NURBS B-rep added later**, a deliberate inversion of the usual order that Keel should treat as a real fork in the road, not ignore; (iii) it treats **SubD and TMSpline as first-class peer representations** to B-rep, matching the corpus's day-one-multi-representation stance. And the Chinese industry's own gap analysis independently confirms file 29's thesis: the differentiator and the bottleneck are both **robust booleans/blends/tolerant modeling on dirty industrial geometry, backed by a large regression corpus.** Independent commercial confirmation (ZWSOFT calling fillet/blend the "Mount Everest" of the kernel) that the corpus prioritized the right subsystem.

2. **The tangent/contact-case intersection iteration (Chen Xiaodiao et al., 2024).** The most directly adoptable *algorithm* found: a derivative-free, convergence-order-2-on-tangency curve-intersection iteration with measured 2x-4x speedups precisely in the contact case that the corpus flags as hardest (file 39). Worth a real evaluation for Keel's intersection code.

3. **The PIA/LSPIA fitting paradigm (Lin Hongwei and successors), especially constrained LSPIA.** A genuinely different fitting paradigm the corpus omitted: iterative control-point feedback instead of a global linear solve, trivially parallel/incremental, with a constrained variant (CG-CLSPIA) that maps onto the exact constrained-surfacing problem of files 32/33. Track as an alternative fitter for interactive/streaming/large-data cases, not a replacement for the KKT solver.

4. **Mu-basis / moving-surface implicitization that survives base points (Falai Chen lineage).** Now correctly identified as a tracked line. Not on Keel's primary parametric+exact-predicate path, but the best exact tool for *rational* surface intersection and base-point-robust classification if Keel ever needs algebraic SSI. The USTC CAGD lineage (Chen's mu-basis → AMCAX's T-mesh kernel) is now legible as a single thread.

5. **Local-refinement splines for IGA (Kang/AMSS and the T-spline community).** Under-represented in the corpus and now strategically relevant because it is the mathematics underneath AMCAX's T-mesh-first kernel. The capability classical NURBS lacks (local refinement without global propagation) is worth a deliberate decision.

**Bottom line for the requester.** The Chinese *journal* literature did not hide a robustness/boolean silver bullet from the English corpus; the corpus already had the substance. The real new yield is (1) the domestic-kernel *engineering and strategy* picture, above all the AMCAX from-scratch account that independently validates Keel's numerical-substrate and architecture choices and surfaces the T-mesh-first path as a serious alternative, and (2) a small number of concrete, recent, adoptable algorithm nuggets (the tangent-case intersection iteration; the constrained-LSPIA fitter). Everything else is corroboration of conclusions the corpus already reached.

---

## References (translated titles)

1. "天工开物: 中望CAD全栈技术架构深度解构与国产工业软件的破壁之路" (Tian Gong Kai Wu: A Deep Deconstruction of ZWCAD's Full-Stack Technical Architecture and the Wall-Breaking Path of Domestic Industrial Software). CSDN, 2024/2025 (fetch-blocked; used via search summary). https://blog.csdn.net/qq_30377315/article/details/149053108
2. ZWSOFT, Overdrive geometric kernel product page (几何内核 Overdrive). https://www.zwsoft.cn/product/overdrive
3. "中望overdrive几何内核合作交流会在沪举办: 共探国产根技术生态共建" (Zhongwang Overdrive Kernel Cooperation Meeting Held in Shanghai). NetEase/163, 2024. https://www.163.com/dy/article/KR7F80BU05568W0A.html
4. "中国CAD稳了!中望软件2026年度产品发布会成功举办" (China's CAD Is Secure: ZWSOFT 2026 Annual Product Launch). 2026. https://m.tech.china.com/redian/2026/0421/042026_1851463.html
5. "国产三维CAD华天软件SiNOVATION 几何造型内核CRUX IV 解析" (Analysis of Huatian SiNOVATION's CRUX IV Geometric Modeling Kernel). CSDN/Zhihu, 2020/2021. https://blog.csdn.net/weixin_43911798/article/details/108745352
6. "华天软件 SINOVATION 9.1 自主可控三维CAD内核CRUX IV 历史由来" (The Origin History of Huatian SINOVATION 9.1's Self-Controllable CRUX IV Kernel). 技术邻 jishulink, post 1816853. https://www.jishulink.com/post/1816853
7. AMCAX (Jiushao) official documentation, v4.7.0 (九韶内核: 简介). https://docs.amcax.net/v4_7_0/zh_cn/html/index.html
8. AMCAX product site (九韶内核 AMCAX). https://product.amcax.net/ ; https://amcax.net/
9. "新一代工业软件内核发布" (Next-Generation Industrial Software Kernel Released), Anhui Center for Applied Mathematics, USTC, 2022. http://acam.ustc.edu.cn/2022/1010/c25550a574875/page.htm
10. "国产完全自主可控三维建模引擎: 九韶内核AMCAX, 首席科学家杨周旺教授" (Fully Self-Controllable Domestic 3D Modeling Engine: AMCAX, Chief Scientist Prof. Yang Zhouwang). Zhihu, p/632573148 (via search summary).
11. "工业软件行业专题报告: 工业软件底层技术剖析" (Industrial Software Sector Report: Anatomy of Industrial Software's Underlying Technology). Tencent News, 2021. https://news.qq.com/rain/a/20211105A02KX200
12. "国内的CAD企业是否都依赖第三方几何内核(如 ACIS, Parasolid等)?" (Do Domestic CAD Companies All Depend on Third-Party Geometry Kernels?). Zhihu Q&A 445615319 (fetch-blocked; via search summary). https://www.zhihu.com/question/445615319
13. "ACIS、Parasolid、OPENCASCADE等几何内核对比" (Comparison of ACIS, Parasolid, OpenCASCADE Kernels). 工业4.0头条 innovation4.cn. http://www.innovation4.cn/toutiao/108119-9903391112/
14. Wang Yongao, Lyu Hangting, Chen Xiaodiao. "An Efficient Method for Computing the Intersection between Two B-Spline Curves" (一种高效的两条B样条曲线求交方法). Journal of Computer-Aided Design & Computer Graphics 36(5), 2024. DOI 10.3724/SP.J.1089.2024.19869. https://www.jcad.cn/cn/article/pdf/preview/10.3724/SP.J.1089.2024.19869.pdf
15. Chen Falai, research pages (μ-基, 动曲面方法, 曲面隐式化与求交). USTC. https://faculty.ustc.edu.cn/chenfalai/zh_CN/zdylm/1025563/list/index.htm
16. Lin Hongwei. "几何迭代法及其应用综述" (A Survey of Geometric Iteration Methods and Their Applications). Journal of Computer-Aided Design & Computer Graphics 27(4), 2015.
17. Fatihu H.Y., Jiang Yini, Lin Hongwei. "Gauss-Seidel最小二乘渐进迭代逼近" (Gauss-Seidel Least-Squares Progressive Iterative Approximation). JCAD. DOI 10.3724/SP.J.1089.2021.18289. https://www.jcad.cn/article/doi/10.3724/SP.J.1089.2021.18289
18. Yang Jinbiao, Sun Mengchen, Hu Qianqian. "基于共轭梯度的约束最小二乘渐进迭代逼近算法" (Conjugate-Gradient-Based Constrained Least-Squares Progressive Iterative Approximation). JCAD. DOI 10.3724/SP.J.1089.2025-00105. https://www.jcad.cn/article/doi/10.3724/SP.J.1089.2025-00105
19. "三角B-B曲面最小二乘渐进迭代格式的革新与加速" (Innovation and Acceleration of the LSPIA Scheme for Triangular Bernstein-Bezier Surfaces). JCAD. DOI 10.3724/SP.J.1089.2022.19010. https://www.jcad.cn/article/doi/10.3724/SP.J.1089.2022.19010
20. "加速的B样条曲线曲面拟合最小二乘渐进迭代逼近" (Accelerated LSPIA for B-spline Curve and Surface Fitting). Journal of Zhejiang University (Science Edition), 2025. https://www.zjujournals.com/sci/
21. Kang Hongmei. "面向等几何分析的局部加细样条" (Local-Refinement Splines for Isogeometric Analysis). AMSS, CAS, 2025. http://www.amss.ac.cn/mzxsbg/202504/t20250418_7603633.html
22. "基于T样条的变网格等几何薄板动力学分析" (Variable-Mesh Isogeometric Thin-Plate Dynamics Analysis Based on T-Splines). Chinese Journal of Theoretical and Applied Mechanics 53(8), 2021. DOI 10.6052/0459-1879-21-199.
23. TPMS geometric-modeling survey (三周期极小曲面几何造型综述). JCAD 35(3), 2023. DOI 10.3724/SP.J.1089.2023.19359.
24. 3D-representation-and-conversion survey (三维模型表示与转换综述). JCAD 37(10), 2025. DOI 10.3724/SP.J.1089.2025-00141. https://www.jcad.cn/en/article/pdf/preview/10.3724/SP.J.1089.2025-00141.pdf
25. Yang Zhifei, Shi Xiquan, Wang Weiming, Liu Xiuping. "A Parameterized Surface Reconstruction Method Based on Powell-Sabin Subdivision" (基于Powell-Sabin细分的参数化曲面重建方法). JCAD 35(12), 2023. DOI 10.3724/SP.J.1089.2023.2023-00021. https://www.jcad.cn/cn/article/pdf/preview/10.3724/SP.J.1089.2023.2023-00021.pdf
