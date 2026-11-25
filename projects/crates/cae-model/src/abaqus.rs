//! Reader for a documented subset of Abaqus `.inp` input files.
//!
//! The reader is intentionally narrow. It recognises the keywords that map
//! onto the truss/beam model (`*Node`, `*Element`, `*Nset`, `*Elset`,
//! `*Material`/`*Elastic`, the three section keywords, `*Boundary`, `*Cload`
//! and `*Dload`) and records every other keyword or option as an
//! [`ImportLoss`] entry instead of dropping it silently. Nodal coordinates are
//! never converted: the caller supplies the unit system.
//!
//! Supported element types are `T3D2` (truss), `B31` (two-dimensional beam),
//! and the solid types `C3D4`, `C3D8` and `C3D8R`. A solid element cannot be
//! represented by this product, so each `*Element` block of a solid type is
//! skipped with one loss entry naming the type and element count. An unknown
//! element type is an error.
//!
//! `*Include` is not resolved: it fails with
//! `import/abaqus-unsupported-include` so a partially imported model never
//! looks complete. `*Part`/`*End Part`/`*Assembly`/`*End Assembly` are treated
//! as structural markers, while `*Instance` is a loss because instance
//! transforms are not applied.

use std::{
    collections::{HashMap, HashSet},
    path::Path,
};

use serde::Serialize;

use crate::{CaeError, ElementKind, ElementMaterial, ElementUpdate, Model, Units};

/// Format tag of the report returned by [`crate::OpenCae::import_abaqus`].
pub const ABAQUS_IMPORT_REPORT_FORMAT: &str = "open-cae-abaqus-import";

/// Version of the Abaqus import report.
pub const ABAQUS_IMPORT_REPORT_VERSION: u32 = 1;

/// Upper bound on the number of ids one `generate` data line may expand to.
const MAX_GENERATED_SET: i64 = 100_000;

/// One Abaqus feature that was recognised but not represented in the document.
///
/// Loss entries are ordered by first occurrence and deduplicated by
/// `(code, keyword, detail)`; `line` is the first line that produced the entry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportLoss {
    /// Stable loss code, for example `import/abaqus-unsupported-keyword`.
    pub code: String,
    /// Keyword that produced the loss, without the leading `*`.
    pub keyword: String,
    /// One-based line number of the keyword or data line.
    pub line: usize,
    /// Human readable explanation of what was ignored.
    pub detail: String,
}

/// Result of reading one Abaqus input file.
#[derive(Debug, Clone)]
pub struct AbaqusImport {
    /// Model described by the imported subset.
    pub model: Model,
    /// Features that were recognised but not represented.
    pub losses: Vec<ImportLoss>,
}

/// Reads `path` as an Abaqus `.inp` file using `units` and returns the model.
pub fn import_abaqus_file(path: &Path, units: Units) -> Result<AbaqusImport, CaeError> {
    let text = std::fs::read_to_string(path)
        .map_err(|error| CaeError::new("io/read", format!("cannot read `{}`: {error}", path.display())))?;
    import_abaqus(&text, units)
}

/// Reads Abaqus input `text` using `units` and returns the model.
///
/// The node dimension is inferred from the coordinates: a model in which every
/// node has `z = 0` becomes two-dimensional, otherwise three-dimensional.
pub fn import_abaqus(text: &str, units: Units) -> Result<AbaqusImport, CaeError> {
    let blocks = tokenize(text)?;
    if let Some(block) = blocks.iter().find(|block| block.keyword == "include") {
        let file = param(block, "input").unwrap_or("?");
        return Err(CaeError::new(
            "import/abaqus-unsupported-include",
            format!("line {}: `*Include` is not supported (requested `{file}`); inline the included file", block.line),
        ));
    }
    let (nodes, dimension) = collect_nodes(&blocks)?;

    let mut model = Model::new_with_dimension(units, dimension);
    for node in &nodes {
        model
            .add_node_3d(&node.id, node.x, node.y, node.z)
            .map_err(|error| CaeError::new(error.code, format!("line {}: {}", node.line, error.message)))?;
    }
    let node_ids: HashSet<String> = nodes.iter().map(|node| node.id.clone()).collect();

    let mut losses = LossLog::default();
    let mut materials: Vec<MaterialDef> = Vec::new();
    let mut current_material: Option<String> = None;
    let mut nsets: HashMap<String, Vec<String>> = HashMap::new();
    let mut elsets: HashMap<String, Vec<String>> = HashMap::new();
    let mut section_defs: Vec<SectionDef> = Vec::new();
    let mut element_defs: Vec<ElementDef> = Vec::new();
    let mut element_ids: HashSet<String> = HashSet::new();
    let mut boundaries: Vec<BoundaryDef> = Vec::new();
    let mut cloads: Vec<CloadDef> = Vec::new();
    let mut dloads: Vec<DloadDef> = Vec::new();

    for block in &blocks {
        match block.keyword.as_str() {
            "node" => {
                check_params(block, &[], &mut losses);
            }
            "nset" | "elset" => {
                read_set(block, &mut nsets, &mut elsets, &mut losses)?;
            }
            "element" => {
                read_elements(block, &node_ids, &mut elsets, &mut element_defs, &mut element_ids, &mut losses)?;
            }
            "material" => {
                check_params(block, &["name"], &mut losses);
                let name = block.name().unwrap_or_else(|| format!("material{}", materials.len() + 1));
                let key = name.to_ascii_lowercase();
                if materials.iter().any(|material| material.key == key) {
                    losses.push(
                        "import/abaqus-duplicate-material",
                        &block.display,
                        block.line,
                        format!("material `{name}` is defined more than once; the later definition wins"),
                    );
                }
                materials.push(MaterialDef { key: key.clone(), name, elastic: None, line: block.line });
                current_material = Some(key);
            }
            "elastic" => {
                read_elastic(block, &current_material, &mut materials, &mut losses)?;
            }
            "solid section" | "truss section" | "beam section" => {
                section_defs.push(read_section(block, &mut losses)?);
            }
            "boundary" => {
                read_boundary(block, &mut boundaries, &mut losses)?;
            }
            "cload" => {
                read_cload(block, &mut cloads, &mut losses)?;
            }
            "dload" => {
                read_dload(block, &mut dloads, &mut losses)?;
            }
            "part" | "end part" | "assembly" | "end assembly" | "end instance" => {}
            "instance" => {
                losses.push(
                    "import/abaqus-unsupported-keyword",
                    &block.display,
                    block.line,
                    format!("`*{}` transforms are not applied; define the geometry inline", block.display),
                );
            }
            "include" => {
                let file = param(block, "input").unwrap_or("?");
                return Err(CaeError::new(
                    "import/abaqus-unsupported-include",
                    format!("line {}: `*Include` is not supported (requested `{file}`); inline the included file", block.line),
                ));
            }
            _ => {
                losses.push(
                    "import/abaqus-unsupported-keyword",
                    &block.display,
                    block.line,
                    format!("keyword `*{}` is not supported and was skipped", block.display),
                );
            }
        }
    }

    let mut material_e: HashMap<String, f64> = HashMap::new();
    let mut first_material: Option<(String, f64)> = None;
    for material in &materials {
        if let Some(elastic) = material.elastic {
            material_e.insert(material.key.clone(), elastic);
            if first_material.is_none() {
                first_material = Some((material.name.clone(), elastic));
            }
        }
    }
    if materials.len() > 1 {
        losses.push(
            "import/abaqus-multiple-materials",
            "Material",
            materials[1].line,
            format!(
                "{} materials are defined; the first with `*Elastic` supplies the global material and each section supplies per-element E",
                materials.len()
            ),
        );
    }
    match &first_material {
        Some((name, youngs_modulus)) => {
            model.material.name = name.clone();
            model.material.youngs_modulus = *youngs_modulus;
        }
        None => {
            if let Some(element) = element_defs.first() {
                losses.push(
                    "import/abaqus-default-material",
                    "Material",
                    element.line,
                    "no `*Elastic` data was found; the default document material is used".to_string(),
                );
            }
        }
    }
    if let Some(area) = section_defs.iter().find_map(|section| match section.kind {
        SectionKind::Truss => section.area,
        _ => None,
    }) {
        model.material.area = area;
    }

    let mut beam_nodes: HashSet<String> = HashSet::new();
    let mut created: Vec<CreatedElement> = Vec::new();
    for element in &element_defs {
        let section = section_defs.iter().find(|section| section.applies(element, &elsets));
        let (area, inertia) = match element.kind {
            ElementKind::Truss => {
                let Some(section) = section.filter(|section| section.kind == SectionKind::Truss && section.area.is_some())
                else {
                    return Err(element_without_section(element, "a `*Truss Section` with an area"));
                };
                (section.area.expect("checked above"), None)
            }
            ElementKind::Beam => {
                if model.dimension != 2 {
                    return Err(CaeError::new(
                        "import/abaqus-unsupported-beam",
                        format!(
                            "line {}: beam element `{}` cannot be represented in a three-dimensional document; B31 is two-dimensional only",
                            element.line, element.id
                        ),
                    ));
                }
                let Some(section) = section.filter(|section| section.kind == SectionKind::Beam && section.inertia.is_some())
                else {
                    return Err(element_without_section(element, "a `*Beam Section` with RECT or CIRC dimensions"));
                };
                (section.area.expect("checked above"), Some(section.inertia.expect("checked above")))
            }
            ElementKind::Beam3D => {
                return Err(CaeError::new(
                    "import/abaqus-unsupported-beam",
                    format!("line {}: beam3d members are not produced by the Abaqus importer", element.line),
                ));
            }
        };
        let youngs_modulus = match section.and_then(|section| section.material.as_deref()) {
            Some(name) => *material_e.get(name).ok_or_else(|| {
                CaeError::new(
                    "import/abaqus-unknown-material",
                    format!(
                        "line {}: section `{}` references material `{name}` without `*Elastic` data",
                        section.expect("section exists").line,
                        section.expect("section exists").elset.as_deref().unwrap_or("<default>")
                    ),
                )
            })?,
            None => model.material.youngs_modulus,
        };
        let e_override = (youngs_modulus != model.material.youngs_modulus).then_some(youngs_modulus);
        let a_override = (area != model.material.area).then_some(area);
        let material = if e_override.is_some() || a_override.is_some() {
            Some(ElementMaterial { youngs_modulus: e_override, area: a_override })
        }
        else {
            None
        };
        model
            .add_element_with_loads(&element.id, &element.from, &element.to, material, element.kind, inertia, None, None)
            .map_err(|error| CaeError::new(error.code, format!("line {}: {}", element.line, error.message)))?;
        if element.kind == ElementKind::Beam {
            beam_nodes.insert(element.from.clone());
            beam_nodes.insert(element.to.clone());
        }
        created.push(CreatedElement { id: element.id.clone(), kind: element.kind });
    }

    for dload in &dloads {
        let targets = if let Some(ids) = elsets.get(&dload.target.to_ascii_lowercase()) {
            ids.clone()
        }
        else if created.iter().any(|element| element.id == dload.target) {
            vec![dload.target.clone()]
        }
        else {
            return Err(CaeError::new(
                "import/abaqus-unknown-element",
                format!("line {}: `*Dload` references unknown element or elset `{}`", dload.line, dload.target),
            ));
        };
        let mut matched = false;
        for id in targets {
            let Some(element) = created.iter().find(|element| element.id == id)
            else {
                continue;
            };
            matched = true;
            match element.kind {
                ElementKind::Beam => {
                    model
                        .update_element_with(
                            &id,
                            &ElementUpdate { distributed_load: Some(dload.magnitude), ..ElementUpdate::default() },
                        )
                        .map_err(|error| CaeError::new(error.code, format!("line {}: {}", dload.line, error.message)))?;
                }
                ElementKind::Truss => losses.push(
                    "import/abaqus-unsupported-load",
                    "Dload",
                    dload.line,
                    "a uniform `PY` load on a truss element is ignored; distributed loads require beam elements".to_string(),
                ),
                ElementKind::Beam3D => losses.push(
                    "import/abaqus-unsupported-load",
                    "Dload",
                    dload.line,
                    "a uniform `PY` load on a beam3d member is ignored by the importer".to_string(),
                ),
            }
        }
        if !matched {
            losses.push(
                "import/abaqus-unsupported-load",
                "Dload",
                dload.line,
                format!("`*Dload` target `{}` contains no imported truss/beam element", dload.target),
            );
        }
    }

    for boundary in &boundaries {
        let targets = resolve_target(&boundary.target, &nsets, &node_ids, "Boundary", boundary.line)?;
        for node in targets {
            let (mut dx, mut dy, mut dz, mut rz) = (false, false, false, false);
            for dof in boundary.first..=boundary.last {
                match dof {
                    1 => dx = true,
                    2 => dy = true,
                    3 if model.dimension == 3 => dz = true,
                    3 => losses.push(
                        "import/abaqus-unsupported-boundary",
                        "Boundary",
                        boundary.line,
                        "a Z translation constraint in a two-dimensional document is ignored".to_string(),
                    ),
                    4 | 5 => losses.push(
                        "import/abaqus-unsupported-boundary",
                        "Boundary",
                        boundary.line,
                        format!("rotation degree of freedom {dof} is not available in the two-dimensional beam model"),
                    ),
                    6 if beam_nodes.contains(&node) => rz = true,
                    6 => losses.push(
                        "import/abaqus-unsupported-boundary",
                        "Boundary",
                        boundary.line,
                        "the rz constraint is ignored at nodes without a beam element".to_string(),
                    ),
                    _ => {}
                }
            }
            if !(dx || dy || dz || rz) {
                continue;
            }
            let result = match boundary.value {
                Some(value) => model.prescribe_displacement(
                    &node,
                    dx.then_some(value),
                    dy.then_some(value),
                    dz.then_some(value),
                    rz.then_some(value),
                ),
                None => model.add_support_full(&node, dx, dy, dz, rz),
            };
            result.map_err(|error| CaeError::new(error.code, format!("line {}: {}", boundary.line, error.message)))?;
        }
    }

    for cload in &cloads {
        let targets = resolve_target(&cload.target, &nsets, &node_ids, "Cload", cload.line)?;
        for node in targets {
            let (fx, fy, fz, mz) = match cload.dof {
                1 => (Some(cload.magnitude), None, None, None),
                2 => (None, Some(cload.magnitude), None, None),
                3 if model.dimension == 3 => (None, None, Some(cload.magnitude), None),
                3 => {
                    losses.push(
                        "import/abaqus-unsupported-load",
                        "Cload",
                        cload.line,
                        "a Z force in a two-dimensional document is ignored".to_string(),
                    );
                    continue;
                }
                6 if beam_nodes.contains(&node) => (None, None, None, Some(cload.magnitude)),
                6 => {
                    losses.push(
                        "import/abaqus-unsupported-load",
                        "Cload",
                        cload.line,
                        "an mz moment at a node without a beam element is ignored".to_string(),
                    );
                    continue;
                }
                _ => {
                    losses.push(
                        "import/abaqus-unsupported-load",
                        "Cload",
                        cload.line,
                        format!("force degree of freedom {} is not available in the two-dimensional beam model", cload.dof),
                    );
                    continue;
                }
            };
            model
                .add_load_full(&node, fx, fy, fz, mz)
                .map_err(|error| CaeError::new(error.code, format!("line {}: {}", cload.line, error.message)))?;
        }
    }

    Ok(AbaqusImport { model, losses: losses.entries })
}

/// One keyword block: the keyword line plus its data lines.
struct Block {
    display: String,
    keyword: String,
    line: usize,
    params: Vec<(String, Option<String>)>,
    data: Vec<(usize, String)>,
}

impl Block {
    fn name(&self) -> Option<String> {
        param(self, "name").map(str::to_string)
    }
}

struct RawNode {
    id: String,
    x: f64,
    y: f64,
    z: f64,
    line: usize,
}

struct MaterialDef {
    key: String,
    name: String,
    line: usize,
    elastic: Option<f64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SectionKind {
    Solid,
    Truss,
    Beam,
}

struct SectionDef {
    kind: SectionKind,
    elset: Option<String>,
    material: Option<String>,
    area: Option<f64>,
    inertia: Option<f64>,
    line: usize,
}

impl SectionDef {
    fn applies(&self, element: &ElementDef, elsets: &HashMap<String, Vec<String>>) -> bool {
        match self.elset.as_deref() {
            None => true,
            Some(name) => elsets.get(name).is_some_and(|ids| ids.iter().any(|id| id == &element.id)),
        }
    }
}

struct ElementDef {
    id: String,
    from: String,
    to: String,
    kind: ElementKind,
    line: usize,
}

struct CreatedElement {
    id: String,
    kind: ElementKind,
}

struct BoundaryDef {
    target: String,
    first: u32,
    last: u32,
    value: Option<f64>,
    line: usize,
}

struct CloadDef {
    target: String,
    dof: u32,
    magnitude: f64,
    line: usize,
}

struct DloadDef {
    target: String,
    magnitude: f64,
    line: usize,
}

#[derive(Default)]
struct LossLog {
    entries: Vec<ImportLoss>,
    seen: HashSet<(String, String, String)>,
}

impl LossLog {
    fn push(&mut self, code: &str, keyword: &str, line: usize, detail: String) {
        let key = (code.to_string(), keyword.to_string(), detail.clone());
        if self.seen.insert(key) {
            self.entries.push(ImportLoss { code: code.to_string(), keyword: keyword.to_string(), line, detail });
        }
    }
}

fn tokenize(text: &str) -> Result<Vec<Block>, CaeError> {
    let mut blocks: Vec<Block> = Vec::new();
    for (index, raw) in text.lines().enumerate() {
        let line = index + 1;
        let trimmed = raw.trim();
        if trimmed.is_empty() || trimmed.starts_with("**") {
            continue;
        }
        if let Some(rest) = trimmed.strip_prefix('*') {
            let mut fields = rest.split(',');
            let display = fields.next().unwrap_or_default().trim().to_string();
            if display.is_empty() {
                return Err(malformed(line, "a `*` line has no keyword name"));
            }
            let mut params = Vec::new();
            for field in fields {
                let field = field.trim();
                if field.is_empty() {
                    continue;
                }
                match field.split_once('=') {
                    Some((key, value)) => {
                        params.push((key.trim().to_ascii_lowercase(), Some(value.trim().trim_matches('"').to_string())))
                    }
                    None => params.push((field.to_ascii_lowercase(), None)),
                }
            }
            blocks.push(Block { keyword: display.to_ascii_lowercase(), display, line, params, data: Vec::new() });
        }
        else {
            let block = blocks.last_mut().ok_or_else(|| malformed(line, "a data line appears before any keyword"))?;
            block.data.push((line, trimmed.to_string()));
        }
    }
    Ok(blocks)
}

fn collect_nodes(blocks: &[Block]) -> Result<(Vec<RawNode>, u32), CaeError> {
    let mut nodes: Vec<RawNode> = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();
    for block in blocks.iter().filter(|block| block.keyword == "node") {
        for (line, data) in &block.data {
            let fields = split_fields(data);
            if fields.len() < 3 || fields.len() > 4 {
                return Err(malformed(
                    *line,
                    format!("a `*Node` line needs an id and 2 or 3 coordinates, found {}", fields.len()),
                ));
            }
            let id = fields[0].to_string();
            if id.is_empty() {
                return Err(malformed(*line, "a `*Node` line has an empty id"));
            }
            if !seen.insert(id.clone()) {
                return Err(CaeError::new(
                    "import/abaqus-duplicate-node",
                    format!("line {line}: node `{id}` is defined more than once"),
                ));
            }
            let x = number(*line, fields[1], "the node x coordinate")?;
            let y = number(*line, fields[2], "the node y coordinate")?;
            let z = match fields.get(3) {
                Some(value) => number(*line, value, "the node z coordinate")?,
                None => 0.0,
            };
            nodes.push(RawNode { id, x, y, z, line: *line });
        }
    }
    if nodes.is_empty() {
        return Err(CaeError::new("import/abaqus-no-nodes", "the input contains no `*Node` data"));
    }
    let dimension = if nodes.iter().any(|node| node.z != 0.0) { 3 } else { 2 };
    Ok((nodes, dimension))
}

fn read_set(
    block: &Block,
    nsets: &mut HashMap<String, Vec<String>>,
    elsets: &mut HashMap<String, Vec<String>>,
    losses: &mut LossLog,
) -> Result<(), CaeError> {
    let name_key = if block.keyword == "nset" { "nset" } else { "elset" };
    let Some(name) = param(block, name_key)
    else {
        return Err(malformed(block.line, format!("`*{}` requires `{name_key}=name`", block.display)));
    };
    check_params(block, &[name_key, "generate"], losses);
    let generate = has_flag(block, "generate");
    let mut ids: Vec<String> = Vec::new();
    for (line, data) in &block.data {
        if generate {
            let fields = split_fields(data);
            if fields.len() < 2 || fields.len() > 3 {
                return Err(malformed(*line, "a generated set line needs start, end and an optional step"));
            }
            let start = integer(*line, fields[0], "the set start")?;
            let end = integer(*line, fields[1], "the set end")?;
            let step = match fields.get(2) {
                Some(value) => integer(*line, value, "the set step")?,
                None => 1,
            };
            if step <= 0 {
                return Err(malformed(*line, "the set generation step must be positive"));
            }
            if end < start {
                continue;
            }
            let count = (end - start) / step + 1;
            if count > MAX_GENERATED_SET {
                return Err(malformed(
                    *line,
                    format!("the generated set would contain {count} ids, above the limit of {MAX_GENERATED_SET}"),
                ));
            }
            let mut value = start;
            while value <= end {
                ids.push(value.to_string());
                value += step;
            }
        }
        else {
            ids.extend(split_fields(data).into_iter().map(str::to_string));
        }
    }
    let key = name.to_ascii_lowercase();
    let target = if block.keyword == "nset" { nsets } else { elsets };
    target.entry(key).or_default().extend(ids);
    Ok(())
}

fn read_elements(
    block: &Block,
    node_ids: &HashSet<String>,
    elsets: &mut HashMap<String, Vec<String>>,
    element_defs: &mut Vec<ElementDef>,
    element_ids: &mut HashSet<String>,
    losses: &mut LossLog,
) -> Result<(), CaeError> {
    check_params(block, &["type", "elset"], losses);
    let Some(raw_type) = param(block, "type")
    else {
        return Err(malformed(block.line, "`*Element` requires `type=`"));
    };
    let kind = raw_type.to_ascii_uppercase();
    let solid = matches!(kind.as_str(), "C3D4" | "C3D8" | "C3D8R");
    if !solid && kind != "T3D2" && kind != "B31" {
        return Err(CaeError::new(
            "import/abaqus-unsupported-element-type",
            format!(
                "line {}: element type `{kind}` is not supported; supported types are C3D4, C3D8, C3D8R, T3D2 and B31",
                block.line
            ),
        ));
    }
    let sets: Vec<String> = block
        .params
        .iter()
        .filter(|(key, _)| key == "elset")
        .filter_map(|(_, value)| value.as_deref())
        .map(str::to_ascii_lowercase)
        .collect();
    for set in &sets {
        elsets.entry(set.clone()).or_default();
    }
    let mut solid_count = 0usize;
    for (line, data) in &block.data {
        let fields = split_fields(data);
        let Some(id) = fields.first()
        else {
            continue;
        };
        let id = id.to_string();
        for set in &sets {
            let ids = elsets.get_mut(set).expect("registered above");
            if !ids.iter().any(|value| value == &id) {
                ids.push(id.clone());
            }
        }
        if solid {
            solid_count += 1;
            continue;
        }
        if fields.len() != 3 {
            return Err(malformed(
                *line,
                format!("`{kind}` element `{id}` needs 2 node references, found {}", fields.len().saturating_sub(1)),
            ));
        }
        let from = fields[1].to_string();
        let to = fields[2].to_string();
        for node in [&from, &to] {
            if !node_ids.contains(node) {
                return Err(CaeError::new(
                    "import/abaqus-unknown-node",
                    format!("line {line}: element `{id}` references unknown node `{node}`"),
                ));
            }
        }
        if !element_ids.insert(id.clone()) {
            return Err(CaeError::new(
                "import/abaqus-duplicate-element",
                format!("line {line}: element `{id}` is defined more than once"),
            ));
        }
        element_defs.push(ElementDef {
            id,
            from,
            to,
            kind: if kind == "T3D2" { ElementKind::Truss } else { ElementKind::Beam },
            line: *line,
        });
    }
    if solid_count > 0 {
        losses.push(
            "import/abaqus-solid-element",
            &block.display,
            block.line,
            format!(
                "{solid_count} solid element(s) of type `{kind}` cannot be represented by this truss/beam document and were skipped"
            ),
        );
    }
    Ok(())
}

fn read_elastic(
    block: &Block,
    current_material: &Option<String>,
    materials: &mut [MaterialDef],
    losses: &mut LossLog,
) -> Result<(), CaeError> {
    check_params(block, &["type"], losses);
    let Some(current) = current_material.as_deref()
    else {
        return Err(malformed(block.line, "`*Elastic` appears before any `*Material`"));
    };
    let elastic_type = param(block, "type").unwrap_or("iso").to_ascii_lowercase();
    if elastic_type != "iso" && elastic_type != "isotropic" {
        losses.push(
            "import/abaqus-unsupported-material",
            &block.display,
            block.line,
            format!("elastic type `{elastic_type}` is not isotropic and was ignored"),
        );
        return Ok(());
    }
    let Some((line, data)) = block.data.first()
    else {
        return Err(malformed(block.line, "`*Elastic` requires a data line with E and Poisson's ratio"));
    };
    let fields = split_fields(data);
    if fields.len() < 2 {
        return Err(malformed(*line, "`*Elastic` needs Young's modulus and Poisson's ratio"));
    }
    let youngs_modulus = number(*line, fields[0], "Young's modulus")?;
    if youngs_modulus <= 0.0 {
        return Err(malformed(*line, "Young's modulus must be positive"));
    }
    let poisson = number(*line, fields[1], "Poisson's ratio")?;
    losses.push(
        "import/abaqus-ignored-material-property",
        &block.display,
        *line,
        format!("Poisson's ratio {poisson} is not used by the truss/beam model"),
    );
    if block.data.len() > 1 {
        losses.push(
            "import/abaqus-ignored-material-data",
            &block.display,
            block.data[1].0,
            "additional elastic data (temperature dependence) is ignored; only the first line is used".to_string(),
        );
    }
    if let Some(material) = materials.iter_mut().find(|material| material.key == current) {
        material.elastic = Some(youngs_modulus);
    }
    Ok(())
}

fn read_section(block: &Block, losses: &mut LossLog) -> Result<SectionDef, CaeError> {
    check_params(block, &["elset", "material", "section"], losses);
    let elset = param(block, "elset").map(str::to_ascii_lowercase);
    let material = param(block, "material").map(str::to_ascii_lowercase);
    match block.keyword.as_str() {
        "solid section" => {
            if let Some((line, data)) = block.data.first() {
                if !split_fields(data).is_empty() {
                    losses.push(
                        "import/abaqus-ignored-section-data",
                        &block.display,
                        *line,
                        "solid-section data (shell thickness) is ignored".to_string(),
                    );
                }
            }
            Ok(SectionDef { kind: SectionKind::Solid, elset, material, area: None, inertia: None, line: block.line })
        }
        "truss section" => {
            let Some((line, data)) = block.data.first()
            else {
                return Err(malformed(block.line, "`*Truss Section` requires an area data line"));
            };
            let fields = split_fields(data);
            let Some(token) = fields.first()
            else {
                return Err(malformed(*line, "`*Truss Section` needs a cross-sectional area"));
            };
            let area = number(*line, token, "the truss area")?;
            if area <= 0.0 {
                return Err(malformed(*line, "the truss area must be positive"));
            }
            if block.data.len() > 1 || fields.len() > 1 {
                losses.push(
                    "import/abaqus-ignored-section-data",
                    &block.display,
                    *line,
                    "additional truss-section data is ignored".to_string(),
                );
            }
            Ok(SectionDef { kind: SectionKind::Truss, elset, material, area: Some(area), inertia: None, line: block.line })
        }
        _ => {
            let Some(section_type) = param(block, "section")
            else {
                return Err(malformed(block.line, "`*Beam Section` requires `section=RECT` or `section=CIRC`"));
            };
            let section_type = section_type.to_ascii_lowercase();
            let Some((line, data)) = block.data.first()
            else {
                return Err(malformed(block.line, "`*Beam Section` requires a cross-section data line"));
            };
            let fields = split_fields(data);
            let (area, inertia) = match section_type.as_str() {
                "rect" => {
                    if fields.len() < 2 {
                        return Err(malformed(*line, "a RECT beam section needs width and height"));
                    }
                    let width = number(*line, fields[0], "the RECT width")?;
                    let height = number(*line, fields[1], "the RECT height")?;
                    if width <= 0.0 || height <= 0.0 {
                        return Err(malformed(*line, "RECT beam width and height must be positive"));
                    }
                    (width * height, width * height.powi(3) / 12.0)
                }
                "circ" => {
                    let Some(token) = fields.first()
                    else {
                        return Err(malformed(*line, "a CIRC beam section needs a radius"));
                    };
                    let radius = number(*line, token, "the CIRC radius")?;
                    if radius <= 0.0 {
                        return Err(malformed(*line, "the CIRC radius must be positive"));
                    }
                    (std::f64::consts::PI * radius * radius, std::f64::consts::PI * radius.powi(4) / 4.0)
                }
                _ => {
                    return Err(CaeError::new(
                        "import/abaqus-unsupported-beam-section",
                        format!(
                            "line {}: beam section type `{section_type}` is not supported; use `RECT` or `CIRC`",
                            block.line
                        ),
                    ));
                }
            };
            if block.data.len() > 1 || fields.len() > 2 {
                losses.push(
                    "import/abaqus-ignored-section-data",
                    &block.display,
                    *line,
                    "beam orientation and additional section data are ignored".to_string(),
                );
            }
            Ok(SectionDef {
                kind: SectionKind::Beam,
                elset,
                material,
                area: Some(area),
                inertia: Some(inertia),
                line: block.line,
            })
        }
    }
}

fn read_boundary(block: &Block, boundaries: &mut Vec<BoundaryDef>, losses: &mut LossLog) -> Result<(), CaeError> {
    check_params(block, &[], losses);
    for (line, data) in &block.data {
        let fields = split_fields(data);
        if fields.len() < 2 {
            return Err(malformed(*line, "`*Boundary` needs a node/set and a degree of freedom"));
        }
        let target = fields[0].to_string();
        if let Ok(first) = fields[1].parse::<u32>() {
            if fields.len() > 4 {
                return Err(malformed(*line, "`*Boundary` has too many fields"));
            }
            let last = match fields.get(2) {
                Some(value) => {
                    value.parse::<u32>().map_err(|_| malformed(*line, format!("`{value}` is not a degree of freedom")))?
                }
                None => first,
            };
            let value = match fields.get(3) {
                Some(token) => Some(number(*line, token, "the prescribed displacement")?),
                None => None,
            };
            if !(1..=6).contains(&first) || !(1..=6).contains(&last) || last < first {
                return Err(malformed(*line, format!("degree-of-freedom range {first}..{last} is outside 1..6")));
            }
            boundaries.push(BoundaryDef { target, first, last, value, line: *line });
        }
        else {
            if fields.len() > 2 {
                return Err(malformed(*line, "a `*Boundary` preset line takes one field after the node/set"));
            }
            let preset = fields[1].to_ascii_uppercase();
            let (first, last) = match preset.as_str() {
                "ENCASTRE" => (1, 6),
                "PINNED" => (1, 3),
                _ => {
                    losses.push(
                        "import/abaqus-unsupported-boundary",
                        &block.display,
                        *line,
                        format!("boundary preset `{preset}` is ignored; use numeric degrees of freedom"),
                    );
                    continue;
                }
            };
            boundaries.push(BoundaryDef { target, first, last, value: None, line: *line });
        }
    }
    Ok(())
}

fn read_cload(block: &Block, cloads: &mut Vec<CloadDef>, losses: &mut LossLog) -> Result<(), CaeError> {
    check_params(block, &[], losses);
    for (line, data) in &block.data {
        let fields = split_fields(data);
        if fields.len() != 3 {
            return Err(malformed(*line, "`*Cload` needs a node/set, a degree of freedom and a magnitude"));
        }
        let dof = integer(*line, fields[1], "the load degree of freedom")?;
        if !(1..=6).contains(&dof) {
            return Err(malformed(*line, format!("load degree of freedom {dof} is outside 1..6")));
        }
        let magnitude = number(*line, fields[2], "the load magnitude")?;
        cloads.push(CloadDef { target: fields[0].to_string(), dof: dof as u32, magnitude, line: *line });
    }
    Ok(())
}

fn read_dload(block: &Block, dloads: &mut Vec<DloadDef>, losses: &mut LossLog) -> Result<(), CaeError> {
    check_params(block, &[], losses);
    for (line, data) in &block.data {
        let fields = split_fields(data);
        if fields.len() != 3 {
            return Err(malformed(*line, "`*Dload` needs an element/set, a load label and a magnitude"));
        }
        let label = fields[1].to_ascii_uppercase();
        if label != "PY" {
            losses.push(
                "import/abaqus-unsupported-load",
                &block.display,
                *line,
                format!("distributed load `{label}` is ignored; only the beam-local `PY` direction is supported"),
            );
            continue;
        }
        let magnitude = number(*line, fields[2], "the distributed load magnitude")?;
        dloads.push(DloadDef { target: fields[0].to_string(), magnitude, line: *line });
    }
    Ok(())
}

fn resolve_target(
    target: &str,
    nsets: &HashMap<String, Vec<String>>,
    node_ids: &HashSet<String>,
    keyword: &str,
    line: usize,
) -> Result<Vec<String>, CaeError> {
    let ids = if let Some(ids) = nsets.get(&target.to_ascii_lowercase()) {
        ids.clone()
    }
    else if node_ids.contains(target) {
        vec![target.to_string()]
    }
    else {
        return Err(CaeError::new(
            "import/abaqus-unknown-node",
            format!("line {line}: `*{keyword}` references unknown node or node set `{target}`"),
        ));
    };
    for id in &ids {
        if !node_ids.contains(id) {
            return Err(CaeError::new(
                "import/abaqus-unknown-node",
                format!("line {line}: node set `{target}` contains unknown node `{id}`"),
            ));
        }
    }
    Ok(ids)
}

fn element_without_section(element: &ElementDef, required: &str) -> CaeError {
    CaeError::new(
        "import/abaqus-element-without-section",
        format!("line {}: element `{}` needs {required} that applies to it", element.line, element.id),
    )
}

fn check_params(block: &Block, allowed: &[&str], losses: &mut LossLog) {
    for (key, value) in &block.params {
        if allowed.contains(&key.as_str()) {
            continue;
        }
        let detail = match value {
            Some(value) => format!("option `{key}={value}` of `*{}` is ignored", block.display),
            None => format!("option `{key}` of `*{}` is ignored", block.display),
        };
        losses.push("import/abaqus-ignored-option", &block.display, block.line, detail);
    }
}

fn param<'a>(block: &'a Block, name: &str) -> Option<&'a str> {
    block.params.iter().find(|(key, _)| key == name).and_then(|(_, value)| value.as_deref())
}

fn has_flag(block: &Block, name: &str) -> bool {
    block.params.iter().any(|(key, value)| key == name && value.is_none())
}

fn split_fields(data: &str) -> Vec<&str> {
    data.split(',').map(str::trim).filter(|field| !field.is_empty()).collect()
}

fn number(line: usize, token: &str, what: &str) -> Result<f64, CaeError> {
    token
        .parse::<f64>()
        .ok()
        .filter(|value| value.is_finite())
        .ok_or_else(|| malformed(line, format!("`{token}` is not a finite number for {what}")))
}

fn integer(line: usize, token: &str, what: &str) -> Result<i64, CaeError> {
    token.parse::<i64>().map_err(|_| malformed(line, format!("`{token}` is not an integer for {what}")))
}

fn malformed(line: usize, detail: impl AsRef<str>) -> CaeError {
    CaeError::new("import/abaqus-malformed-line", format!("line {line}: {}", detail.as_ref()))
}
