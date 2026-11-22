//! Built-in material presets and parametric cross-section properties.
//!
//! Presets are stored in the millimetre system (`MPa`, `t/mm³`) and converted
//! on demand, so a model created in metres receives the same physical values.
//! Section formulas use the local axes of a two-dimensional beam: the strong
//! axis is bending about `z` with depth along `y`.

use serde::Serialize;

use crate::{
    CaeError,
    model::{Material, Section, Units},
};

/// One built-in isotropic linear-elastic material.
///
/// Values are held in the millimetre system: Young's modulus and permissible
/// stress in `MPa`, density in `t/mm³`. Convert with [`MaterialPreset::in_units`]
/// or build a [`Material`] directly with [`MaterialPreset::material`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MaterialPreset {
    /// Machine readable preset name used by the CLI and SDK.
    pub name: &'static str,
    /// Young's modulus in MPa.
    pub youngs_modulus_mpa: f64,
    /// Mass density in t/mm³.
    pub density_t_per_mm3: f64,
    /// Permissible stress in MPa, when the preset defines one.
    pub allowable_stress_mpa: Option<f64>,
}

/// Built-in material presets.
pub const MATERIAL_PRESETS: &[MaterialPreset] = &[
    MaterialPreset {
        name: "steel",
        youngs_modulus_mpa: 210_000.0,
        density_t_per_mm3: 7.85e-9,
        allowable_stress_mpa: Some(235.0),
    },
    MaterialPreset {
        name: "stainless-steel",
        youngs_modulus_mpa: 193_000.0,
        density_t_per_mm3: 7.90e-9,
        allowable_stress_mpa: Some(200.0),
    },
    MaterialPreset {
        name: "aluminum",
        youngs_modulus_mpa: 70_000.0,
        density_t_per_mm3: 2.70e-9,
        allowable_stress_mpa: Some(110.0),
    },
    MaterialPreset { name: "concrete", youngs_modulus_mpa: 30_000.0, density_t_per_mm3: 2.40e-9, allowable_stress_mpa: None },
    MaterialPreset {
        name: "timber",
        youngs_modulus_mpa: 11_000.0,
        density_t_per_mm3: 6.00e-10,
        allowable_stress_mpa: Some(10.0),
    },
];

impl MaterialPreset {
    /// Young's modulus in the requested unit system.
    pub fn youngs_modulus(self, units: Units) -> f64 {
        match units {
            Units::Millimeter => self.youngs_modulus_mpa,
            Units::Meter => self.youngs_modulus_mpa * 1.0e6,
        }
    }

    /// Mass density in the requested unit system (`t/mm³` or `kg/m³`).
    pub fn density(self, units: Units) -> f64 {
        match units {
            Units::Millimeter => self.density_t_per_mm3,
            Units::Meter => self.density_t_per_mm3 * 1.0e12,
        }
    }

    /// Permissible stress in the requested unit system.
    pub fn allowable_stress(self, units: Units) -> Option<f64> {
        self.allowable_stress_mpa.map(|value| match units {
            Units::Millimeter => value,
            Units::Meter => value * 1.0e6,
        })
    }

    /// Builds a complete global material for the requested unit system.
    pub fn material(self, units: Units, area: f64, section: Option<Section>) -> Material {
        Material {
            name: self.name.to_string(),
            youngs_modulus: self.youngs_modulus(units),
            area,
            density: self.density(units),
            allowable_stress: self.allowable_stress(units),
            alpha: None,
            section,
        }
    }
}

/// Looks up a material preset by its exact (lowercase) name.
pub fn material_preset(name: &str) -> Result<MaterialPreset, CaeError> {
    MATERIAL_PRESETS.iter().copied().find(|preset| preset.name == name).ok_or_else(|| {
        let available = MATERIAL_PRESETS.iter().map(|preset| preset.name).collect::<Vec<_>>().join(", ");
        CaeError::new("material/unknown-preset", format!("unknown material preset `{name}`; valid presets are {available}"))
    })
}

/// Area, bending inertias, torsion constant and extreme-fibre distance of a
/// parametric section.
///
/// `inertia_strong` is bending about the local `z` axis and is the value the
/// two-dimensional beam solver uses. `extreme_fiber` is measured from the
/// neutral axis to the farthest fibre, so `section_modulus()` equals
/// `inertia_strong / extreme_fiber`.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SectionProperties {
    /// Cross-sectional area.
    pub area: f64,
    /// Second moment of area about the strong (bending) axis.
    pub inertia_strong: f64,
    /// Second moment of area about the weak axis.
    pub inertia_weak: f64,
    /// Saint-Venant torsion constant.
    pub torsion_constant: f64,
    /// Distance from the neutral axis to the extreme fibre.
    pub extreme_fiber: f64,
}

impl SectionProperties {
    /// Elastic section modulus `I_strong / c`.
    pub fn section_modulus(&self) -> f64 {
        self.inertia_strong / self.extreme_fiber
    }

    /// Radius of gyration about the strong (bending) axis, `sqrt(I_strong / A)`.
    pub fn radius_of_gyration_strong(&self) -> f64 {
        (self.inertia_strong / self.area).sqrt()
    }

    /// Radius of gyration about the weak axis, `sqrt(I_weak / A)`.
    pub fn radius_of_gyration_weak(&self) -> f64 {
        (self.inertia_weak / self.area).sqrt()
    }
}

/// Derived properties of one parametric section, as reported by
/// [`section_report`].
///
/// `units` labels the unit system the input dimensions were given in; the
/// properties are `length²` (area), `length⁴` (inertias and torsion constant)
/// and `length` (extreme fibre, section modulus and radii of gyration).
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SectionReport {
    /// Shape identifier, matching the `shape` tag of the section.
    pub shape: String,
    /// Unit system label (`mm` or `m`) the dimensions were interpreted in.
    pub units: String,
    /// Cross-sectional area.
    pub area: f64,
    /// Second moment of area about the strong (bending) axis.
    pub inertia_strong: f64,
    /// Second moment of area about the weak axis.
    pub inertia_weak: f64,
    /// Saint-Venant torsion constant.
    pub torsion_constant: f64,
    /// Distance from the neutral axis to the extreme fibre.
    pub extreme_fiber: f64,
    /// Elastic section modulus `I_strong / c`.
    pub section_modulus: f64,
    /// Radius of gyration about the strong axis.
    pub radius_of_gyration_strong: f64,
    /// Radius of gyration about the weak axis.
    pub radius_of_gyration_weak: f64,
}

/// Computes the derived properties of a parametric section.
///
/// The geometry is validated exactly like [`Section::properties`], so invalid
/// dimensions report `material/invalid-section` instead of non-finite values.
pub fn section_report(section: Section, units: Units) -> Result<SectionReport, CaeError> {
    let properties = section.properties()?;
    Ok(SectionReport {
        shape: section.shape_name().to_string(),
        units: units.as_str().to_string(),
        area: properties.area,
        inertia_strong: properties.inertia_strong,
        inertia_weak: properties.inertia_weak,
        torsion_constant: properties.torsion_constant,
        extreme_fiber: properties.extreme_fiber,
        section_modulus: properties.section_modulus(),
        radius_of_gyration_strong: properties.radius_of_gyration_strong(),
        radius_of_gyration_weak: properties.radius_of_gyration_weak(),
    })
}

impl Section {
    /// Validates the geometry and returns its properties.
    ///
    /// Invalid dimensions are reported as `material/invalid-section` instead of
    /// producing zero or non-finite properties.
    pub fn properties(self) -> Result<SectionProperties, CaeError> {
        match self {
            Self::Rectangle { width, height } => {
                validate_dimensions("rectangle", &[("width", width), ("height", height)])?;
                let (large, small) = if width >= height { (width, height) } else { (height, width) };
                let ratio = small / large;
                let torsion = large * small.powi(3) / 3.0 * (1.0 - 0.63 * ratio + 0.052 * ratio.powi(5));
                Ok(SectionProperties {
                    area: width * height,
                    inertia_strong: width * height.powi(3) / 12.0,
                    inertia_weak: height * width.powi(3) / 12.0,
                    torsion_constant: torsion,
                    extreme_fiber: height / 2.0,
                })
            }
            Self::Circle { diameter } => {
                validate_dimensions("circle", &[("diameter", diameter)])?;
                Ok(SectionProperties {
                    area: std::f64::consts::PI * diameter.powi(2) / 4.0,
                    inertia_strong: std::f64::consts::PI * diameter.powi(4) / 64.0,
                    inertia_weak: std::f64::consts::PI * diameter.powi(4) / 64.0,
                    torsion_constant: std::f64::consts::PI * diameter.powi(4) / 32.0,
                    extreme_fiber: diameter / 2.0,
                })
            }
            Self::ISection { depth, flange_width, flange_thickness, web_thickness } => {
                validate_dimensions(
                    "i",
                    &[
                        ("depth", depth),
                        ("flangeWidth", flange_width),
                        ("flangeThickness", flange_thickness),
                        ("webThickness", web_thickness),
                    ],
                )?;
                let web_height = depth - 2.0 * flange_thickness;
                if web_height <= 0.0 {
                    return Err(invalid_section("i-section flange thickness must be smaller than half the depth"));
                }
                if web_thickness >= flange_width {
                    return Err(invalid_section("i-section web thickness must be smaller than the flange width"));
                }
                Ok(SectionProperties {
                    area: 2.0 * flange_width * flange_thickness + web_height * web_thickness,
                    inertia_strong: (flange_width * depth.powi(3) - (flange_width - web_thickness) * web_height.powi(3)) / 12.0,
                    inertia_weak: (2.0 * flange_thickness * flange_width.powi(3) + web_height * web_thickness.powi(3)) / 12.0,
                    torsion_constant: (2.0 * flange_width * flange_thickness.powi(3) + web_height * web_thickness.powi(3))
                        / 3.0,
                    extreme_fiber: depth / 2.0,
                })
            }
            Self::BoxSection { width, height, wall } => {
                validate_dimensions("box", &[("width", width), ("height", height), ("wall", wall)])?;
                let inner_width = width - 2.0 * wall;
                let inner_height = height - 2.0 * wall;
                if width <= 2.0 * wall || height <= 2.0 * wall || wall >= width.min(height) / 2.0 {
                    return Err(invalid_section(
                        "box section wall thickness must be smaller than half of the width and half of the height",
                    ));
                }
                Ok(SectionProperties {
                    area: width * height - inner_width * inner_height,
                    inertia_strong: (width * height.powi(3) - inner_width * inner_height.powi(3)) / 12.0,
                    inertia_weak: (height * width.powi(3) - inner_height * inner_width.powi(3)) / 12.0,
                    torsion_constant: 2.0 * wall * (width - wall).powi(2) * (height - wall).powi(2)
                        / (width + height - 2.0 * wall),
                    extreme_fiber: height / 2.0,
                })
            }
            Self::Tube { outer_diameter, inner_diameter } => {
                validate_dimensions("tube", &[("outerDiameter", outer_diameter), ("innerDiameter", inner_diameter)])?;
                if outer_diameter <= inner_diameter {
                    return Err(invalid_section("tube outer diameter must be greater than its inner diameter"));
                }
                let difference = outer_diameter.powi(4) - inner_diameter.powi(4);
                Ok(SectionProperties {
                    area: std::f64::consts::PI / 4.0 * (outer_diameter.powi(2) - inner_diameter.powi(2)),
                    inertia_strong: std::f64::consts::PI / 64.0 * difference,
                    inertia_weak: std::f64::consts::PI / 64.0 * difference,
                    torsion_constant: std::f64::consts::PI / 32.0 * difference,
                    extreme_fiber: outer_diameter / 2.0,
                })
            }
            Self::Channel { depth, flange_width, flange_thickness, web_thickness } => {
                validate_dimensions(
                    "channel",
                    &[
                        ("depth", depth),
                        ("flangeWidth", flange_width),
                        ("flangeThickness", flange_thickness),
                        ("webThickness", web_thickness),
                    ],
                )?;
                let web_height = depth - 2.0 * flange_thickness;
                if web_height <= 0.0 {
                    return Err(invalid_section("channel section flange thickness must be smaller than half the depth"));
                }
                if web_thickness >= flange_width {
                    return Err(invalid_section("channel section web thickness must be smaller than the flange width"));
                }
                let area = 2.0 * flange_width * flange_thickness + web_height * web_thickness;
                let centroid_x =
                    (flange_width * flange_thickness * flange_width + web_height * web_thickness.powi(2) / 2.0) / area;
                let flange_offset = depth / 2.0 - flange_thickness / 2.0;
                let inertia_strong = 2.0
                    * (flange_width * flange_thickness.powi(3) / 12.0
                        + flange_width * flange_thickness * flange_offset.powi(2))
                    + web_thickness * web_height.powi(3) / 12.0;
                let inertia_weak = 2.0
                    * (flange_thickness * flange_width.powi(3) / 12.0
                        + flange_width * flange_thickness * (flange_width / 2.0 - centroid_x).powi(2))
                    + (web_height * web_thickness.powi(3) / 12.0
                        + web_height * web_thickness * (centroid_x - web_thickness / 2.0).powi(2));
                let torsion_constant =
                    (2.0 * flange_width * flange_thickness.powi(3) + web_height * web_thickness.powi(3)) / 3.0;
                Ok(SectionProperties { area, inertia_strong, inertia_weak, torsion_constant, extreme_fiber: depth / 2.0 })
            }
        }
    }
}

impl Section {
    /// Creates and validates a solid rectangular section.
    pub fn rectangle(width: f64, height: f64) -> Result<Self, CaeError> {
        let section = Self::Rectangle { width, height };
        section.properties()?;
        Ok(section)
    }

    /// Creates and validates a solid circular section.
    pub fn circle(diameter: f64) -> Result<Self, CaeError> {
        let section = Self::Circle { diameter };
        section.properties()?;
        Ok(section)
    }

    /// Creates and validates a doubly symmetric I-section.
    pub fn i_section(depth: f64, flange_width: f64, flange_thickness: f64, web_thickness: f64) -> Result<Self, CaeError> {
        let section = Self::ISection { depth, flange_width, flange_thickness, web_thickness };
        section.properties()?;
        Ok(section)
    }

    /// Creates and validates a hollow rectangular box section.
    pub fn box_section(width: f64, height: f64, wall: f64) -> Result<Self, CaeError> {
        let section = Self::BoxSection { width, height, wall };
        section.properties()?;
        Ok(section)
    }

    /// Creates and validates a hollow circular tube.
    pub fn tube(outer_diameter: f64, inner_diameter: f64) -> Result<Self, CaeError> {
        let section = Self::Tube { outer_diameter, inner_diameter };
        section.properties()?;
        Ok(section)
    }

    /// Creates and validates an open channel (C-section).
    pub fn channel(depth: f64, flange_width: f64, flange_thickness: f64, web_thickness: f64) -> Result<Self, CaeError> {
        let section = Self::Channel { depth, flange_width, flange_thickness, web_thickness };
        section.properties()?;
        Ok(section)
    }
}

/// Returns the built-in library as pretty-printed JSON in the requested units.
pub fn library_json(units: Units) -> Result<String, CaeError> {
    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    struct PresetView {
        name: &'static str,
        youngs_modulus: f64,
        density: f64,
        allowable_stress: Option<f64>,
    }

    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    struct ShapeView {
        shape: &'static str,
        parameters: Vec<&'static str>,
    }

    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    struct Library {
        units: &'static str,
        materials: Vec<PresetView>,
        sections: Vec<ShapeView>,
    }

    let library = Library {
        units: units.as_str(),
        materials: MATERIAL_PRESETS
            .iter()
            .map(|preset| PresetView {
                name: preset.name,
                youngs_modulus: preset.youngs_modulus(units),
                density: preset.density(units),
                allowable_stress: preset.allowable_stress(units),
            })
            .collect(),
        sections: vec![
            ShapeView { shape: "rectangle", parameters: vec!["width", "height"] },
            ShapeView { shape: "circle", parameters: vec!["diameter"] },
            ShapeView { shape: "i", parameters: vec!["depth", "flangeWidth", "flangeThickness", "webThickness"] },
            ShapeView { shape: "box", parameters: vec!["width", "height", "wall"] },
            ShapeView { shape: "tube", parameters: vec!["outerDiameter", "innerDiameter"] },
            ShapeView { shape: "channel", parameters: vec!["depth", "flangeWidth", "flangeThickness", "webThickness"] },
        ],
    };
    serde_json::to_string_pretty(&library)
        .map_err(|error| CaeError::new("document/serialize", format!("cannot serialize the library: {error}")))
}

fn validate_dimensions(shape: &str, dimensions: &[(&str, f64)]) -> Result<(), CaeError> {
    for (name, value) in dimensions {
        if !value.is_finite() || *value <= 0.0 {
            return Err(invalid_section(format!("{shape} section `{name}` must be positive and finite (currently {value})")));
        }
    }
    Ok(())
}

fn invalid_section(message: impl Into<String>) -> CaeError {
    CaeError::new("material/invalid-section", message)
}
