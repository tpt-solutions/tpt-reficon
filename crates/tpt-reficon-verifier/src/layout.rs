//! Memory-layout and alignment proof logic for FFI contracts.
//!
//! Adapted from TPT Solutions' `tpt-telos-uir-bridge`: that bridge proves
//! that a scope's symbolic memory allocations fit a hardware budget using the
//! Fourier–Motzkin engine. The same idea is applied here to struct layout — a
//! `repr(C)` struct is a sequence of fields whose byte extents must be
//! *within* the struct and *non-overlapping*, and the whole struct must
//! mirror a foreign (C/C++/Go) layout byte-for-byte.
//!
//! Two layers:
//!
//! 1. [`StructLayout::compute`] — derives offsets/size/alignment from a
//!    field list using the standard C `repr(C)` algorithm (fields aligned to
//!    their own alignment, struct size padded up to the maximum field
//!    alignment).
//! 2. [`prove_equivalent`] — a fully deterministic equality proof against a
//!    foreign layout, and [`prove_within_bounds`] — a *solver-backed* proof
//!    (via [`crate::solver::entails`]) that every field extent lies inside
//!    the struct and adjacent extents do not overlap.

use crate::ir::{Constraint, Linear, Relation};
use crate::solver::entails;

/// A byte alignment: `2^k` for some `k`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Alignment(u32);

impl Alignment {
    /// The trivial `alignof(())` = 1.
    pub const BYTE: Alignment = Alignment(1);
    /// Two-byte alignment (`alignof(u16)`).
    pub const WORD: Alignment = Alignment(2);
    /// Four-byte alignment (`alignof(u32)`).
    pub const DWORD: Alignment = Alignment(4);
    /// Eight-byte alignment (`alignof(u64)`).
    pub const QWORD: Alignment = Alignment(8);

    /// Returns `None` when `bytes` is not a positive power of two.
    pub const fn new(bytes: u32) -> Option<Alignment> {
        if bytes.is_power_of_two() && bytes > 0 {
            Some(Alignment(bytes))
        } else {
            None
        }
    }

    /// The alignment in bytes.
    pub const fn bytes(self) -> u32 {
        self.0
    }
}

/// A field type descriptor used to derive a `repr(C)` layout.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LayoutField {
    /// Field identifier (used for diagnostics only).
    pub name: String,
    /// Size of the field's type in bytes.
    pub size: u64,
    /// Alignment of the field's type.
    pub align: Alignment,
}

/// A fully computed struct layout.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StructLayout {
    /// Fields in declaration order.
    pub fields: Vec<LayoutField>,
    /// Byte offset of each field, parallel to `fields`; the first field
    /// starts at offset 0.
    pub offsets: Vec<u64>,
    /// Total size of the struct in bytes (padded to its alignment).
    pub size: u64,
    /// Alignment of the struct (maximum of its fields' alignments).
    pub align: Alignment,
}

fn align_up(v: u64, align: u32) -> Option<u64> {
    let a = align as u64;
    v.checked_add(a - 1)?.checked_div(a)?.checked_mul(a)
}

impl StructLayout {
    /// Derives the `repr(C)` layout for a field list using the standard C
    /// rules:
    ///
    /// 1. each field's offset is `cursor` rounded up to the field's
    ///    alignment (the first field is placed at offset 0);
    /// 2. `cursor` advances by the field's size;
    /// 3. the struct's alignment is the maximum field alignment;
    /// 4. the struct's total size is `cursor` rounded up to that alignment.
    ///
    /// Returns `None` when any arithmetic would overflow (callers treat this
    /// as "cannot prove", never as a passing layout).
    pub fn compute(fields: &[LayoutField]) -> Option<StructLayout> {
        if fields.is_empty() {
            return Some(StructLayout {
                fields: Vec::new(),
                offsets: Vec::new(),
                size: 0,
                align: Alignment::BYTE,
            });
        }
        let mut offsets = Vec::with_capacity(fields.len());
        let mut cursor: u64 = 0;
        let mut max_align: Alignment = Alignment::BYTE;
        for f in fields {
            let aligned = align_up(cursor, f.align.bytes())?;
            cursor = aligned.checked_add(f.size)?;
            offsets.push(aligned);
            max_align = if f.align.bytes() > max_align.bytes() {
                f.align
            } else {
                max_align
            };
        }
        let size = align_up(cursor, max_align.bytes())?;
        Some(StructLayout {
            fields: fields.to_vec(),
            offsets,
            size,
            align: max_align,
        })
    }

    /// Byte offset of the named field, if present.
    pub fn offset_of(&self, name: &str) -> Option<u64> {
        self.fields
            .iter()
            .position(|f| f.name == name)
            .map(|i| self.offsets[i])
    }

    /// Byte past the end of the field at `index`.
    pub fn field_end(&self, index: usize) -> Option<u64> {
        self.offsets
            .get(index)
            .zip(self.fields.get(index).map(|f| f.size))
            .and_then(|(&o, s)| o.checked_add(s))
    }
}

/// Why a layout proof failed. Structured so proc-macro diagnostics can render
/// span-accurate messages.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LayoutMismatch {
    /// Rust-side layout derivation overflowed (sizes/offsets too large).
    RustLayoutOverflow,
    /// A value needed by the QF_LRA engine exceeds its `i64` range.
    OutOfSolverRange(String),
    /// The two structs have different field counts.
    FieldCountMismatch {
        /// Field count on the Rust side.
        rust: usize,
        /// Field count on the foreign side.
        foreign: usize,
    },
    /// Fields at the same index have different names.
    FieldNameMismatch {
        /// Position of the mismatching field.
        index: usize,
        /// Name on the Rust side.
        rust: String,
        /// Name on the foreign side.
        foreign: String,
    },
    /// A field sits at different offsets.
    OffsetMismatch {
        /// Position of the mismatching field.
        index: usize,
        /// Offset on the Rust side, in bytes.
        rust: u64,
        /// Offset on the foreign side, in bytes.
        foreign: u64,
    },
    /// A field has a different size.
    FieldSizeMismatch {
        /// Position of the mismatching field.
        index: usize,
        /// Size on the Rust side, in bytes.
        rust: u64,
        /// Size on the foreign side, in bytes.
        foreign: u64,
    },
    /// The structs have different alignment.
    AlignmentMismatch {
        /// Alignment on the Rust side, in bytes.
        rust: u32,
        /// Alignment on the foreign side, in bytes.
        foreign: u32,
    },
    /// The structs have different total size.
    StructSizeMismatch {
        /// Total size on the Rust side, in bytes.
        rust: u64,
        /// Total size on the foreign side, in bytes.
        foreign: u64,
    },
    /// A field's extent (Solver proof) extends past the struct or overlaps
    /// the following field.
    ExtentExceedsStruct {
        /// Position of the offending field.
        index: usize,
    },
    /// The offset list does not have one entry per field.
    OffsetsLengthMismatch {
        /// Number of fields.
        fields: usize,
        /// Number of offsets supplied.
        offsets: usize,
    },
    /// A field's offset is not a multiple of its alignment.
    MisalignedField {
        /// Position of the misaligned field.
        index: usize,
        /// The offset, in bytes.
        offset: u64,
        /// The field's alignment, in bytes.
        align: u32,
    },
    /// The struct's total size is not a multiple of its alignment.
    MisalignedSize {
        /// The struct size, in bytes.
        size: u64,
        /// The struct alignment, in bytes.
        align: u32,
    },
    /// The struct's alignment is not the maximum of its fields' alignments.
    StructAlignMismatch {
        /// The maximum field alignment, in bytes.
        expected: u32,
        /// The alignment the layout declares, in bytes.
        found: u32,
    },
    /// A struct with no fields has a non-zero size.
    EmptyStructSize {
        /// The declared size, in bytes.
        size: u64,
    },
}

impl core::fmt::Display for LayoutMismatch {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            LayoutMismatch::RustLayoutOverflow => write!(
                f,
                "rust layout derivation overflowed (sizes/offsets too large)"
            ),
            LayoutMismatch::OutOfSolverRange(what) => {
                write!(f, "{what} exceeds the solver's i64 range")
            }
            LayoutMismatch::FieldCountMismatch { rust, foreign } => {
                write!(f, "field count mismatch: rust {rust} vs foreign {foreign}")
            }
            LayoutMismatch::FieldNameMismatch {
                index,
                rust,
                foreign,
            } => {
                write!(f, "field #{index} name mismatch: `{rust}` vs `{foreign}`")
            }
            LayoutMismatch::OffsetMismatch {
                index,
                rust,
                foreign,
            } => {
                write!(
                    f,
                    "field #{index} offset mismatch: rust {rust}B vs foreign {foreign}B"
                )
            }
            LayoutMismatch::FieldSizeMismatch {
                index,
                rust,
                foreign,
            } => {
                write!(
                    f,
                    "field #{index} size mismatch: rust {rust}B vs foreign {foreign}B"
                )
            }
            LayoutMismatch::AlignmentMismatch { rust, foreign } => {
                write!(
                    f,
                    "struct alignment mismatch: rust {rust} vs foreign {foreign}"
                )
            }
            LayoutMismatch::StructSizeMismatch { rust, foreign } => {
                write!(
                    f,
                    "struct size mismatch: rust {rust}B vs foreign {foreign}B"
                )
            }
            LayoutMismatch::ExtentExceedsStruct { index } => {
                write!(
                    f,
                    "field #{index} extent exceeds struct bounds (or overlaps the next field)"
                )
            }
            LayoutMismatch::OffsetsLengthMismatch { fields, offsets } => {
                write!(f, "{offsets} offsets supplied for {fields} fields")
            }
            LayoutMismatch::MisalignedField {
                index,
                offset,
                align,
            } => {
                write!(
                    f,
                    "field #{index} at offset {offset}B is not aligned to {align}B"
                )
            }
            LayoutMismatch::MisalignedSize { size, align } => {
                write!(
                    f,
                    "struct size {size}B is not a multiple of its {align}B alignment"
                )
            }
            LayoutMismatch::StructAlignMismatch { expected, found } => {
                write!(
                    f,
                    "struct alignment {found} does not match the maximum field alignment {expected}"
                )
            }
            LayoutMismatch::EmptyStructSize { size } => {
                write!(f, "a struct with no fields must have size 0, not {size}B")
            }
        }
    }
}

impl std::error::Error for LayoutMismatch {}

/// Proves that the Rust `repr(C)` layout derived from `rust_fields` mirrors
/// `foreign` byte-for-byte: same field count, names, offsets, sizes, and the
/// same overall alignment and size.
///
/// This is the deterministic check that backs `#[ffi_contract(...)]`: the
/// foreign layout comes from a parsed C/Go header, and this function will
/// only return `Ok` when every byte of both structs is provably identical.
pub fn prove_equivalent(
    rust_fields: &[LayoutField],
    foreign: &StructLayout,
) -> Result<(), LayoutMismatch> {
    let rust = StructLayout::compute(rust_fields).ok_or(LayoutMismatch::RustLayoutOverflow)?;
    if rust.fields.len() != foreign.fields.len() {
        return Err(LayoutMismatch::FieldCountMismatch {
            rust: rust.fields.len(),
            foreign: foreign.fields.len(),
        });
    }
    // Indexed below, so a foreign layout with short offsets must not panic.
    if foreign.offsets.len() != foreign.fields.len() {
        return Err(LayoutMismatch::OffsetsLengthMismatch {
            fields: foreign.fields.len(),
            offsets: foreign.offsets.len(),
        });
    }
    for i in 0..rust.fields.len() {
        let r = &rust.fields[i];
        let fo = &foreign.fields[i];
        if r.name != fo.name {
            return Err(LayoutMismatch::FieldNameMismatch {
                index: i,
                rust: r.name.clone(),
                foreign: fo.name.clone(),
            });
        }
        if r.size != fo.size {
            return Err(LayoutMismatch::FieldSizeMismatch {
                index: i,
                rust: r.size,
                foreign: fo.size,
            });
        }
        if rust.offsets[i] != foreign.offsets[i] {
            return Err(LayoutMismatch::OffsetMismatch {
                index: i,
                rust: rust.offsets[i],
                foreign: foreign.offsets[i],
            });
        }
    }
    if rust.align != foreign.align {
        return Err(LayoutMismatch::AlignmentMismatch {
            rust: rust.align.bytes(),
            foreign: foreign.align.bytes(),
        });
    }
    if rust.size != foreign.size {
        return Err(LayoutMismatch::StructSizeMismatch {
            rust: rust.size,
            foreign: foreign.size,
        });
    }
    Ok(())
}

/// Builds the QF_LRA premises encoding a concrete layout: every field offset,
/// its size, and the struct's total size pinned to their computed constants.
fn layout_premises(layout: &StructLayout) -> Option<Vec<Constraint>> {
    let mut out = Vec::new();
    for (i, (&off, f)) in layout.offsets.iter().zip(&layout.fields).enumerate() {
        let off_var = Linear::var(format!("off_{i}"));
        let size_var = Linear::var(format!("size_{i}"));
        out.push(Constraint(
            off_var.checked_add(&Linear::constant_only(-(off as i64)))?,
            Relation::Eq,
        ));
        out.push(Constraint(
            size_var.checked_add(&Linear::constant_only(-(f.size as i64)))?,
            Relation::Eq,
        ));
    }
    out.push(Constraint(
        Linear::var("total_size").checked_add(&Linear::constant_only(-(layout.size as i64)))?,
        Relation::Eq,
    ));
    Some(out)
}

fn i64_for(name: &str, v: u64) -> Result<i64, LayoutMismatch> {
    i64::try_from(v).map_err(|_| LayoutMismatch::OutOfSolverRange(name.to_string()))
}

/// Solver-backed proof that every field's byte extent is contained within the
/// struct and adjacent field extents do not overlap. The FM engine discharges
/// `⊢ off_i + size_i <= total_size` and `⊢ off_{i-1} + size_{i-1} <= off_i`
/// under the concrete layout premises, mirroring how `tpt-telos-uir-bridge`
/// proves allocation budgets fit their scope.
///
/// Returns `Ok(())` only when every such entailment is *proven*; a
/// conservative failure (overflow, out-of-range) surfaces as a
/// [`LayoutMismatch`], never as a silent pass.
pub fn prove_within_bounds(layout: &StructLayout) -> Result<(), LayoutMismatch> {
    if layout.offsets.len() != layout.fields.len() {
        return Err(LayoutMismatch::OffsetsLengthMismatch {
            fields: layout.fields.len(),
            offsets: layout.offsets.len(),
        });
    }
    // No fields means no extents to prove: the only obligation is size 0.
    let Some(last) = layout.fields.len().checked_sub(1) else {
        return match layout.size {
            0 => Ok(()),
            size => Err(LayoutMismatch::EmptyStructSize { size }),
        };
    };

    // Guard i64 range explicitly first so the `-` conversions in
    // `layout_premises` can never silently wrap.
    for (i, (&off, f)) in layout.offsets.iter().zip(&layout.fields).enumerate() {
        i64_for(&format!("off_{i}"), off)?;
        i64_for(&format!("size_{i}"), f.size)?;
    }
    i64_for("total_size", layout.size)?;

    let premises = layout_premises(layout).ok_or(LayoutMismatch::RustLayoutOverflow)?;
    let var = |name: String| Linear::var(name);
    let overflow = || LayoutMismatch::RustLayoutOverflow;

    for i in 0..layout.fields.len() {
        // off_i + size_i <= total_size
        let fits = var(format!("off_{i}"))
            .checked_add(&var(format!("size_{i}")))
            .and_then(|e| e.checked_sub(&var("total_size".into())))
            .ok_or_else(overflow)?;
        if !entails(&premises, &Constraint(fits, Relation::Le)) {
            return Err(LayoutMismatch::ExtentExceedsStruct { index: i });
        }

        // off_{i-1} + size_{i-1} <= off_i  (previous extent precedes this one)
        if i > 0 {
            let prev = var(format!("off_{}", i - 1))
                .checked_add(&var(format!("size_{}", i - 1)))
                .and_then(|e| e.checked_sub(&var(format!("off_{i}"))))
                .ok_or_else(overflow)?;
            if !entails(&premises, &Constraint(prev, Relation::Le)) {
                return Err(LayoutMismatch::ExtentExceedsStruct { index: i });
            }
        }
    }

    // The first field must start at byte 0, and the tail extent after the last
    // field must not be negative: `total_size - off_last - size_last >= 0`.
    if !entails(&premises, &Constraint(var("off_0".into()), Relation::Ge)) {
        return Err(LayoutMismatch::ExtentExceedsStruct { index: 0 });
    }
    let tail = var("total_size".into())
        .checked_sub(&var(format!("off_{last}")))
        .and_then(|e| e.checked_sub(&var(format!("size_{last}"))))
        .ok_or_else(overflow)?;
    if !entails(&premises, &Constraint(tail, Relation::Ge)) {
        return Err(LayoutMismatch::ExtentExceedsStruct { index: last });
    }

    // Alignment, checked after the extents so overlap keeps its diagnosis.
    // Each offset must be a multiple of its field's alignment, the struct's
    // alignment must be the maximum field alignment, and the size must be a
    // multiple of that alignment.
    for (i, (&off, f)) in layout.offsets.iter().zip(&layout.fields).enumerate() {
        if off % u64::from(f.align.bytes()) != 0 {
            return Err(LayoutMismatch::MisalignedField {
                index: i,
                offset: off,
                align: f.align.bytes(),
            });
        }
    }
    let expected = layout
        .fields
        .iter()
        .map(|f| f.align.bytes())
        .max()
        .unwrap_or(1);
    if layout.align.bytes() != expected {
        return Err(LayoutMismatch::StructAlignMismatch {
            expected,
            found: layout.align.bytes(),
        });
    }
    if layout.size % u64::from(layout.align.bytes()) != 0 {
        return Err(LayoutMismatch::MisalignedSize {
            size: layout.size,
            align: layout.align.bytes(),
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn field(name: &str, size: u64, align: u32) -> LayoutField {
        LayoutField {
            name: name.to_string(),
            align: Alignment::new(align).unwrap(),
            size,
        }
    }

    fn foreign(fields: &[LayoutField]) -> StructLayout {
        StructLayout::compute(fields).unwrap()
    }

    #[test]
    fn alignment_rejects_non_power_of_two() {
        assert!(Alignment::new(0).is_none());
        assert!(Alignment::new(3).is_none());
        assert_eq!(Alignment::new(8), Some(Alignment::QWORD));
    }

    #[test]
    fn c_layout_padding_rules() {
        // { u8, u64, u8 } — the classic padding minefield:
        // off: 0, 8, 16 ; struct align 8 ; size 24.
        let l = foreign(&[field("a", 1, 1), field("b", 8, 8), field("c", 1, 1)]);
        assert_eq!(l.offsets, vec![0, 8, 16]);
        assert_eq!(l.align, Alignment::QWORD);
        assert_eq!(l.size, 24);
        assert_eq!(l.offset_of("b"), Some(8));
        assert_eq!(l.field_end(0), Some(1));
    }

    #[test]
    fn empty_struct() {
        let l = foreign(&[]);
        assert_eq!(l.size, 0);
        assert_eq!(l.align, Alignment::BYTE);
    }

    #[test]
    fn equivalent_layouts_pass() {
        let a = [field("id", 8, 8), field("tag", 1, 1), field("pad4", 4, 4)];
        let foreign_l = foreign(&a);
        assert_eq!(prove_equivalent(&a, &foreign_l), Ok(()));
    }

    #[test]
    fn ordering_is_required() {
        // Same field *types* in a different order is a different layout.
        let a = [field("x", 8, 8), field("y", 1, 1)];
        let b = [field("y", 1, 1), field("x", 8, 8)];
        let foreign_l = foreign(&b);
        let err = prove_equivalent(&a, &foreign_l).unwrap_err();
        assert_eq!(
            err,
            LayoutMismatch::FieldNameMismatch {
                index: 0,
                rust: "x".into(),
                foreign: "y".into()
            }
        );
    }

    #[test]
    fn size_alignment_mismatches_caught() {
        let foreign_l = foreign(&[field("id", 8, 8)]);
        // Rust declares u32 (4B) for a field the C side reserves 8B for.
        let err = prove_equivalent(&[field("id", 4, 4)], &foreign_l).unwrap_err();
        assert_eq!(
            err,
            LayoutMismatch::FieldSizeMismatch {
                index: 0,
                rust: 4,
                foreign: 8
            }
        );
    }

    #[test]
    fn solver_proves_bounds() {
        let l = foreign(&[field("a", 1, 1), field("b", 8, 8), field("c", 1, 1)]);
        assert_eq!(prove_within_bounds(&l), Ok(()));
    }

    #[test]
    fn solver_detects_extent_overflow() {
        // A crafted "foreign" layout whose third field overlaps the second.
        let l = StructLayout {
            fields: vec![field("a", 1, 1), field("b", 8, 8), field("c", 4, 4)],
            offsets: vec![0, 8, 10], // c starts inside b's extent
            size: 20,
            align: Alignment::QWORD,
        };
        let err = prove_within_bounds(&l).unwrap_err();
        assert_eq!(err, LayoutMismatch::ExtentExceedsStruct { index: 2 });
    }

    #[test]
    fn rank_ordering_regression() {
        // Regression guard for the Ge/Gt constant flip fixed in the solver:
        // a layout with a bare `off_0 >= 0` style constraint proves.
        let l = foreign(&[field("only", 4, 4)]);
        assert_eq!(prove_within_bounds(&l), Ok(()));
    }

    #[test]
    fn empty_struct_proves_within_bounds() {
        assert_eq!(prove_within_bounds(&foreign(&[])), Ok(()));
        let sized = StructLayout {
            fields: Vec::new(),
            offsets: Vec::new(),
            size: 4,
            align: Alignment::BYTE,
        };
        assert_eq!(
            prove_within_bounds(&sized),
            Err(LayoutMismatch::EmptyStructSize { size: 4 })
        );
    }

    #[test]
    fn misaligned_offset_is_rejected() {
        // The u32 extent fits and overlaps nothing, but starts at byte 1.
        let l = StructLayout {
            fields: vec![field("a", 1, 1), field("b", 4, 4)],
            offsets: vec![0, 1],
            size: 8,
            align: Alignment::DWORD,
        };
        assert_eq!(
            prove_within_bounds(&l),
            Err(LayoutMismatch::MisalignedField {
                index: 1,
                offset: 1,
                align: 4
            })
        );
    }

    #[test]
    fn misaligned_size_is_rejected() {
        let l = StructLayout {
            fields: vec![field("a", 4, 4)],
            offsets: vec![0],
            size: 6,
            align: Alignment::DWORD,
        };
        assert_eq!(
            prove_within_bounds(&l),
            Err(LayoutMismatch::MisalignedSize { size: 6, align: 4 })
        );
    }

    #[test]
    fn short_offsets_are_rejected_without_panicking() {
        let fields = vec![field("a", 1, 1), field("b", 1, 1)];
        let short = StructLayout {
            fields: fields.clone(),
            offsets: vec![0],
            size: 2,
            align: Alignment::BYTE,
        };
        let expected = LayoutMismatch::OffsetsLengthMismatch {
            fields: 2,
            offsets: 1,
        };
        assert_eq!(prove_within_bounds(&short), Err(expected.clone()));
        // Before the length check, `prove_equivalent` indexed past the end.
        assert_eq!(prove_equivalent(&fields, &short), Err(expected));
    }
}
