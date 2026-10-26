//! Primitives provide building blocks for Specta-based libraries.

use std::{borrow::Cow, collections::HashSet, fmt::Write as _};

use specta::{
    Types,
    datatype::{
        DataType, Enum, Fields, GenericReference, List, Map, NamedDataType, NamedReference,
        NamedReferenceType, OpaqueReference, Primitive, Reference, Struct, Tuple,
    },
};
// PATCH(2)：别名渲染删除后本文件不再使用 `specta-typescript`（改造点见
// `export_single_internal`）。

use crate::{Error, Layout, Zod, map_keys, opaque, reserved_names::RESERVED_TYPE_NAMES};

pub(crate) type TypeRenderStack = Vec<(Cow<'static, str>, Cow<'static, str>)>;

// PATCH(1)：上游的 `specta:strict_object` 标记全生态无写入方（specta / specta-serde /
// tauri-specta 三方零命中），struct 面因此恒为 `z.object`。本仓库要求 strict 写死
// （不做 strict / loose 双套产物），故连同标记常量与其读取分支一并删除。
const OPTIONAL_FLATTEN_UNION_MARKER: &str = "specta_serde:optional_flatten_union";

fn named_reference_generics(r: &NamedReference) -> Result<&[(GenericReference, DataType)], Error> {
    match &r.inner {
        NamedReferenceType::Reference { generics, .. } => Ok(generics),
        NamedReferenceType::Inline { .. } => Ok(&[]),
        NamedReferenceType::Recursive(_) => Err(Error::dangling_named_reference(format!(
            "recursive inline named reference {r:?}"
        ))),
    }
}

fn named_reference_ty<'a>(types: &'a Types, r: &'a NamedReference) -> Result<&'a DataType, Error> {
    match &r.inner {
        NamedReferenceType::Reference { .. } => types
            .get(r)
            .and_then(|ndt| ndt.ty.as_ref())
            .ok_or_else(|| Error::dangling_named_reference(format!("{r:?}"))),
        NamedReferenceType::Inline { dt, .. } => Ok(dt),
        NamedReferenceType::Recursive(_) => Err(Error::dangling_named_reference(format!(
            "recursive inline named reference {r:?}"
        ))),
    }
}

/// Generate a group of `export const XSchema = ...` declarations for named types.
pub fn export<'a>(
    exporter: &dyn AsRef<Zod>,
    types: &Types,
    ndts: impl Iterator<Item = &'a NamedDataType>,
    indent: &str,
) -> Result<String, Error> {
    let mut s = String::new();
    let mut type_render_stack = TypeRenderStack::new();
    export_internal(
        &mut s,
        exporter.as_ref(),
        types,
        ndts,
        indent,
        &mut type_render_stack,
    )?;
    Ok(s)
}

pub(crate) fn export_internal<'a>(
    s: &mut String,
    exporter: &Zod,
    types: &Types,
    ndts: impl Iterator<Item = &'a NamedDataType>,
    indent: &str,
    type_render_stack: &mut TypeRenderStack,
) -> Result<(), Error> {
    for (index, ndt) in ndts.enumerate() {
        if index != 0 {
            s.push('\n');
        }
        export_single_internal(s, exporter, types, ndt, indent, type_render_stack)?;
    }

    Ok(())
}

fn export_single_internal(
    s: &mut String,
    exporter: &Zod,
    types: &Types,
    ndt: &NamedDataType,
    indent: &str,
    type_render_stack: &mut TypeRenderStack,
) -> Result<(), Error> {
    let base_name = exported_type_name(exporter, ndt);
    let name_path = if ndt.module_path.is_empty() {
        ndt.name.to_string()
    } else {
        format!("{}::{}", ndt.module_path, ndt.name)
    };
    validate_type_name(&base_name, name_path.clone())?;
    let schema_name = format!("{base_name}Schema");

    let Some(ty) = &ndt.ty else {
        return Ok(());
    };

    type_render_stack.push((ndt.module_path.clone(), ndt.name.clone()));

    let result = (|| {
        // PATCH(2)：上游在此段渲染 `export type X = …`（`z.infer` 的载体）并在
        // 具名 schema 上标注 `: z.ZodType<X>`；`z.infer` 在递归类型上塌成 `any`
        // 并静默传播，故整段（含 `typescript_alias_datatype` 等辅助）删除：
        // 本产物零 `export type`、零类型注解，类型主权留在 `bindings.ts`。
        // schema 表达式本身与别名渲染无关，直接渲染。
        if ndt.generics.is_empty() {
            let mut schema_expr = String::new();
            datatype(
                &mut schema_expr,
                exporter,
                types,
                ty,
                vec![ndt.name.clone()],
                &[],
                false,
                false,
                type_render_stack,
            )?;
            let schema_expr = indent_continuations(&schema_expr, indent);

            writeln!(s, "{indent}export const {schema_name} = {schema_expr};")?;
            return Ok(());
        }

        let map_key_generics = named_map_key_parameters(ndt, types);
        let mut generic_params = Vec::with_capacity(ndt.generics.len());
        let mut fn_params = Vec::with_capacity(ndt.generics.len());
        let mut overload_params = Vec::with_capacity(ndt.generics.len());
        let mut first_default = None;
        for (index, generic) in ndt.generics.iter().enumerate() {
            let name = generic.name.as_ref();
            validate_type_name(name, format!("{name_path}.<generic {name}>"))?;

            let mut generic_group = vec![format!("{name} extends z.ZodType")];
            let mut fn_group = Vec::new();
            let mut overload_group = vec![format!("{name}: {name}")];
            if let Some(default) = &generic.default {
                first_default.get_or_insert(index);
                let mut default_schema = String::new();
                datatype(
                    &mut default_schema,
                    exporter,
                    types,
                    default,
                    vec![ndt.name.clone(), format!("<generic {name} default>").into()],
                    &[],
                    false,
                    false,
                    type_render_stack,
                )?;
                fn_group.push(format!("{name}: z.ZodType = {default_schema}"));
                if map_key_generics.contains(name) {
                    let key_name = map_key_generic_name(name);
                    map_keys::validate_map_key(
                        default,
                        types,
                        format!("{name_path}.<generic {name} default key>"),
                    )?;
                    let mut default_key_schema = String::new();
                    map_key_datatype(
                        &mut default_key_schema,
                        exporter,
                        types,
                        default,
                        vec![
                            ndt.name.clone(),
                            format!("<generic {name} default key>").into(),
                        ],
                        &[],
                        type_render_stack,
                    )?;
                    generic_group.push(format!("{key_name} extends z.ZodType<PropertyKey>"));
                    overload_group.push(format!("{key_name}: {key_name}"));
                    fn_group.push(format!(
                        "{key_name}: z.ZodType<PropertyKey> = {default_key_schema}"
                    ));
                }
            } else {
                fn_group.push(format!("{name}: {name}"));
                if map_key_generics.contains(name) {
                    let key_name = map_key_generic_name(name);
                    generic_group.push(format!("{key_name} extends z.ZodType<PropertyKey>"));
                    overload_group.push(format!("{key_name}: {key_name}"));
                    fn_group.push(format!("{key_name}: {key_name}"));
                }
            }
            generic_params.push(generic_group);
            fn_params.push(fn_group);
            overload_params.push(overload_group);
        }

        let mut schema_expr = String::new();
        datatype(
            &mut schema_expr,
            exporter,
            types,
            ty,
            vec![ndt.name.clone()],
            &[],
            false,
            false,
            type_render_stack,
        )?;
        let schema_expr = indent_continuations(&schema_expr, indent);

        if let Some(first_default) = first_default {
            for argument_count in first_default..=ndt.generics.len() {
                let generics = generic_params[..argument_count]
                    .iter()
                    .flatten()
                    .cloned()
                    .collect::<Vec<_>>()
                    .join(", ");
                let params = overload_params[..argument_count]
                    .iter()
                    .flatten()
                    .cloned()
                    .collect::<Vec<_>>()
                    .join(", ");
                // PATCH(2)：重载签名同样去掉 `: z.ZodType<TypeName<…>>` 类型实参
                // （`TypeName` 别名已不再存在），统一回到裸 `z.ZodType`。
                let generics = if generics.is_empty() {
                    String::new()
                } else {
                    format!("<{generics}>")
                };
                writeln!(
                    s,
                    "{indent}export function {schema_name}{generics}({params}): z.ZodType;"
                )?;
            }
            writeln!(
                s,
                "{indent}export function {schema_name}({}): z.ZodType<any> {{\n{indent}\treturn {schema_expr};\n{indent}}}",
                fn_params
                    .into_iter()
                    .flatten()
                    .collect::<Vec<_>>()
                    .join(", ")
            )?;
        } else {
            let generic_params = generic_params
                .into_iter()
                .flatten()
                .collect::<Vec<_>>()
                .join(", ");
            let fn_params = fn_params
                .into_iter()
                .flatten()
                .collect::<Vec<_>>()
                .join(", ");
            writeln!(
                s,
                "{indent}export const {schema_name} = <{generic_params}>({fn_params}) => {schema_expr};"
            )?;
        }

        Ok(())
    })();

    type_render_stack.pop();
    result
}

fn indent_continuations<'a>(value: &'a str, indent: &str) -> Cow<'a, str> {
    if indent.is_empty() || !value.contains('\n') {
        Cow::Borrowed(value)
    } else {
        Cow::Owned(value.replace('\n', &format!("\n{indent}")))
    }
}

// PATCH(2)：随 `export type` 别名渲染一并删除的 `typescript_alias_datatype` /
// `typescript_alias_fields` / `resolved_typescript_map_key_alias` 三个辅助原本只为
// 「让 alias 里的 zod-define 引用可 TypeScript 化」而存在，产物 schema 不受影响。

fn map_key_generic_name(generic: &str) -> String {
    format!("$key${generic}")
}

fn generic_map_key_parameters(dt: &DataType, types: &Types) -> HashSet<String> {
    fn collect_fields(
        fields: &Fields,
        types: &Types,
        map_key: bool,
        generics: &[(GenericReference, DataType)],
        stack: &mut Vec<NamedReference>,
        result: &mut HashSet<String>,
    ) {
        match fields {
            Fields::Unit => {}
            Fields::Unnamed(fields) => fields
                .fields
                .iter()
                .filter_map(|field| field.ty.as_ref())
                .for_each(|dt| collect(dt, types, map_key, generics, stack, result)),
            Fields::Named(fields) => fields
                .fields
                .iter()
                .filter_map(|(_, field)| field.ty.as_ref())
                .for_each(|dt| collect(dt, types, map_key, generics, stack, result)),
        }
    }

    fn collect(
        dt: &DataType,
        types: &Types,
        map_key: bool,
        generics: &[(GenericReference, DataType)],
        stack: &mut Vec<NamedReference>,
        result: &mut HashSet<String>,
    ) {
        match dt {
            DataType::Primitive(_) | DataType::Reference(Reference::Opaque(_)) => {}
            DataType::Generic(generic) => {
                if let Some((_, replacement)) =
                    generics.iter().find(|(candidate, _)| candidate == generic)
                    && !matches!(replacement, DataType::Generic(candidate) if candidate == generic)
                {
                    collect(replacement, types, map_key, generics, stack, result);
                } else if map_key {
                    result.insert(generic.name().to_string());
                }
            }
            DataType::List(list) => collect(&list.ty, types, map_key, generics, stack, result),
            DataType::Map(map) => {
                collect(map.key_ty(), types, true, generics, stack, result);
                collect(map.value_ty(), types, false, generics, stack, result);
            }
            DataType::Nullable(inner) => collect(inner, types, map_key, generics, stack, result),
            DataType::Struct(strct) => {
                collect_fields(&strct.fields, types, map_key, generics, stack, result)
            }
            DataType::Enum(enm) => enm.variants.iter().for_each(|(_, variant)| {
                collect_fields(&variant.fields, types, map_key, generics, stack, result)
            }),
            DataType::Tuple(tuple) => tuple
                .elements
                .iter()
                .for_each(|dt| collect(dt, types, map_key, generics, stack, result)),
            DataType::Intersection(types_) => types_
                .iter()
                .for_each(|dt| collect(dt, types, map_key, generics, stack, result)),
            DataType::Reference(Reference::Named(reference)) => match &reference.inner {
                NamedReferenceType::Inline { dt, .. } => {
                    collect(dt, types, map_key, generics, stack, result)
                }
                NamedReferenceType::Reference { .. } => {
                    if stack.contains(reference) {
                        return;
                    }
                    let Some(ty) = types.get(reference).and_then(|ndt| ndt.ty.as_ref()) else {
                        return;
                    };
                    let Ok(reference_generics) = resolved_reference_generics(reference, generics)
                    else {
                        return;
                    };
                    stack.push(reference.clone());
                    collect(ty, types, map_key, &reference_generics, stack, result);
                    stack.pop();
                }
                NamedReferenceType::Recursive(_) => {}
            },
        }
    }

    let mut result = HashSet::new();
    collect(dt, types, false, &[], &mut Vec::new(), &mut result);
    result
}

fn named_map_key_parameters(ndt: &NamedDataType, types: &Types) -> HashSet<String> {
    fn collect_generic_references(dt: &DataType, result: &mut HashSet<String>) {
        match dt {
            DataType::Primitive(_) | DataType::Reference(Reference::Opaque(_)) => {}
            DataType::Generic(generic) => {
                result.insert(generic.name().to_string());
            }
            DataType::List(list) => collect_generic_references(&list.ty, result),
            DataType::Map(map) => {
                collect_generic_references(map.key_ty(), result);
                collect_generic_references(map.value_ty(), result);
            }
            DataType::Nullable(inner) => collect_generic_references(inner, result),
            DataType::Struct(strct) => collect_field_generic_references(&strct.fields, result),
            DataType::Enum(enm) => enm
                .variants
                .iter()
                .for_each(|(_, variant)| collect_field_generic_references(&variant.fields, result)),
            DataType::Tuple(tuple) => tuple
                .elements
                .iter()
                .for_each(|dt| collect_generic_references(dt, result)),
            DataType::Intersection(types) => types
                .iter()
                .for_each(|dt| collect_generic_references(dt, result)),
            DataType::Reference(Reference::Named(reference)) => match &reference.inner {
                NamedReferenceType::Inline { dt, .. } => collect_generic_references(dt, result),
                NamedReferenceType::Reference { generics, .. } => generics
                    .iter()
                    .for_each(|(_, dt)| collect_generic_references(dt, result)),
                NamedReferenceType::Recursive(_) => {}
            },
        }
    }

    fn collect_field_generic_references(fields: &Fields, result: &mut HashSet<String>) {
        match fields {
            Fields::Unit => {}
            Fields::Unnamed(fields) => fields
                .fields
                .iter()
                .filter_map(|field| field.ty.as_ref())
                .for_each(|dt| collect_generic_references(dt, result)),
            Fields::Named(fields) => fields
                .fields
                .iter()
                .filter_map(|(_, field)| field.ty.as_ref())
                .for_each(|dt| collect_generic_references(dt, result)),
        }
    }

    let mut result = ndt
        .ty
        .as_ref()
        .map(|ty| generic_map_key_parameters(ty, types))
        .unwrap_or_default();
    loop {
        let previous_len = result.len();
        for generic in ndt.generics.iter() {
            if result.contains(generic.name.as_ref())
                && let Some(default) = &generic.default
            {
                collect_generic_references(default, &mut result);
            }
        }
        if result.len() == previous_len {
            break;
        }
    }
    result
}

/// Generate an inline Zod expression for a [`DataType`].
///
/// If you are using a custom format such as `specta_serde::Format`, this helper does not apply
/// datatype mapping automatically. Map both the full [`Types`] graph and any top-level
/// [`DataType`] values before calling this helper.
pub fn inline(exporter: &dyn AsRef<Zod>, types: &Types, dt: &DataType) -> Result<String, Error> {
    let mut s = String::new();
    let mut type_render_stack = TypeRenderStack::new();
    datatype(
        &mut s,
        exporter.as_ref(),
        types,
        dt,
        vec![],
        &[],
        false,
        false,
        &mut type_render_stack,
    )?;
    Ok(s)
}

/// Generate a Zod expression for a [`Reference`].
///
/// If you are using a custom format such as `specta_serde::Format`, this helper does not apply
/// datatype mapping automatically.
pub fn reference(exporter: &dyn AsRef<Zod>, types: &Types, r: &Reference) -> Result<String, Error> {
    let mut s = String::new();
    let mut type_render_stack = TypeRenderStack::new();
    reference_dt(
        &mut s,
        exporter.as_ref(),
        types,
        r,
        vec![],
        &[],
        false,
        &mut type_render_stack,
    )?;
    Ok(s)
}

pub(crate) fn datatype_with_inline_attr(
    s: &mut String,
    exporter: &Zod,
    types: &Types,
    dt: &DataType,
    location: Vec<Cow<'static, str>>,
    generics: &[(GenericReference, DataType)],
    inline: bool,
    deferred: bool,
    type_render_stack: &mut TypeRenderStack,
) -> Result<(), Error> {
    datatype(
        s,
        exporter,
        types,
        dt,
        location,
        generics,
        inline,
        deferred,
        type_render_stack,
    )
}

fn datatype(
    s: &mut String,
    exporter: &Zod,
    types: &Types,
    dt: &DataType,
    location: Vec<Cow<'static, str>>,
    generics: &[(GenericReference, DataType)],
    force_inline_ref: bool,
    deferred: bool,
    type_render_stack: &mut TypeRenderStack,
) -> Result<(), Error> {
    match dt {
        DataType::Primitive(p) => s.push_str(primitive_dt(p, location)?),
        DataType::List(l) => list_dt(
            s,
            exporter,
            types,
            l,
            location,
            generics,
            deferred,
            type_render_stack,
        )?,
        DataType::Map(m) => map_dt(
            s,
            exporter,
            types,
            m,
            location,
            generics,
            deferred,
            type_render_stack,
        )?,
        DataType::Nullable(def) => {
            let mut inner = String::new();
            datatype(
                &mut inner,
                exporter,
                types,
                def,
                location,
                generics,
                force_inline_ref,
                deferred,
                type_render_stack,
            )?;
            write!(s, "{inner}.nullable()")?;
        }
        DataType::Struct(st) => struct_dt(
            s,
            exporter,
            types,
            st,
            location,
            generics,
            deferred,
            type_render_stack,
        )?,
        DataType::Enum(enm) => enum_dt(
            s,
            exporter,
            types,
            enm,
            location,
            generics,
            deferred,
            type_render_stack,
            &[],
        )?,
        DataType::Tuple(tuple) => tuple_dt(
            s,
            exporter,
            types,
            tuple,
            location,
            generics,
            deferred,
            type_render_stack,
        )?,
        DataType::Reference(r) => {
            if force_inline_ref {
                match r {
                    Reference::Named(named) => {
                        let ty = named_reference_ty(types, named)?;
                        let reference_generics = named_reference_generics(named)?;
                        datatype(
                            s,
                            exporter,
                            types,
                            ty,
                            location,
                            reference_generics,
                            false,
                            deferred,
                            type_render_stack,
                        )?;
                    }
                    _ => reference_dt(
                        s,
                        exporter,
                        types,
                        r,
                        location,
                        generics,
                        deferred,
                        type_render_stack,
                    )?,
                }
            } else {
                reference_dt(
                    s,
                    exporter,
                    types,
                    r,
                    location,
                    generics,
                    deferred,
                    type_render_stack,
                )?;
            }
        }
        DataType::Generic(g) => generic_dt(s, g),
        DataType::Intersection(intersection) => {
            if intersection.is_empty() {
                s.push_str("z.unknown()");
                return Ok(());
            }
            let allowed_object_keys = intersection
                .iter()
                .filter_map(|ty| match ty {
                    DataType::Struct(strct) => match &strct.fields {
                        Fields::Named(named) => Some(named),
                        _ => None,
                    },
                    _ => None,
                })
                .flat_map(|named| {
                    named
                        .fields
                        .iter()
                        .filter(|(_, field)| field.ty.is_some())
                        .map(|(name, _)| name.as_ref())
                })
                .collect::<Vec<_>>();
            let mut parts = Vec::with_capacity(intersection.len());
            for ty in intersection {
                let mut part = String::new();
                if let DataType::Enum(enm) = ty {
                    enum_dt(
                        &mut part,
                        exporter,
                        types,
                        enm,
                        location.clone(),
                        generics,
                        deferred,
                        type_render_stack,
                        &allowed_object_keys,
                    )?;
                } else if let DataType::Reference(Reference::Named(named)) = ty
                    && !matches!(&named.inner, NamedReferenceType::Recursive(_))
                    && let DataType::Enum(enm) = named_reference_ty(types, named)?
                {
                    let reference_generics = named_reference_generics(named)?;
                    let mut resolved = DataType::Enum(enm.clone());
                    substitute_generics(&mut resolved, reference_generics);
                    let DataType::Enum(resolved) = resolved else {
                        unreachable!("enum generic substitution preserves the datatype kind")
                    };
                    enum_dt(
                        &mut part,
                        exporter,
                        types,
                        &resolved,
                        location.clone(),
                        generics,
                        deferred,
                        type_render_stack,
                        &allowed_object_keys,
                    )?;
                } else {
                    datatype(
                        &mut part,
                        exporter,
                        types,
                        ty,
                        location.clone(),
                        generics,
                        false,
                        deferred,
                        type_render_stack,
                    )?;
                }
                parts.push(part);
            }
            s.push_str(&parts.join(".and("));
            for _ in 1..parts.len() {
                s.push(')');
            }
        }
    }

    Ok(())
}

fn primitive_dt(p: &Primitive, location: Vec<Cow<'static, str>>) -> Result<&'static str, Error> {
    use Primitive::*;

    Ok(match p {
        i8 => "z.int().min(-128).max(127)",
        i16 => "z.int().min(-32768).max(32767)",
        i32 => "z.int().min(-2147483648).max(2147483647)",
        u8 => "z.int().min(0).max(255)",
        u16 => "z.int().min(0).max(65535)",
        u32 => "z.int().min(0).max(4294967295)",
        // JSON serializers encode non-finite floats as `null`.
        f16 | f32 | f64 => "z.number().nullable()",
        usize | isize | i64 | u64 | i128 | u128 | f128 => {
            return Err(Error::bigint_forbidden(location.join(".")));
        }
        Primitive::bool => "z.boolean()",
        str => "z.string()",
        char => "z.string().refine((value) => [...value].length === 1)",
    })
}

fn list_dt(
    s: &mut String,
    exporter: &Zod,
    types: &Types,
    l: &List,
    location: Vec<Cow<'static, str>>,
    generics: &[(GenericReference, DataType)],
    deferred: bool,
    type_render_stack: &mut TypeRenderStack,
) -> Result<(), Error> {
    let mut dt = String::new();
    datatype(
        &mut dt,
        exporter,
        types,
        &l.ty,
        location,
        generics,
        false,
        deferred,
        type_render_stack,
    )?;

    if let Some(length) = l.length {
        s.push_str("z.tuple([");
        for n in 0..length {
            if n != 0 {
                s.push_str(", ");
            }
            s.push_str(&dt);
        }
        s.push_str("])");
    } else {
        write!(s, "z.array({dt})")?;
    }

    Ok(())
}

fn map_dt(
    s: &mut String,
    exporter: &Zod,
    types: &Types,
    m: &Map,
    location: Vec<Cow<'static, str>>,
    generics: &[(GenericReference, DataType)],
    deferred: bool,
    type_render_stack: &mut TypeRenderStack,
) -> Result<(), Error> {
    let mut resolved_key = m.key_ty().clone();
    substitute_generics(&mut resolved_key, generics);
    map_keys::validate_map_key(
        &resolved_key,
        types,
        child_location(&location, "<key>").join("."),
    )?;

    let mut key = String::new();
    map_key_datatype(
        &mut key,
        exporter,
        types,
        m.key_ty(),
        child_location(&location, "<key>"),
        generics,
        type_render_stack,
    )?;
    let mut value = String::new();
    datatype(
        &mut value,
        exporter,
        types,
        m.value_ty(),
        child_location(&location, "<value>"),
        generics,
        false,
        deferred,
        type_render_stack,
    )?;

    let constructor = if map_key_is_finite(m.key_ty(), types, generics) {
        "z.partialRecord"
    } else {
        "z.record"
    };
    write!(s, "{constructor}({key}, {value})")?;
    Ok(())
}

fn map_key_is_finite(
    dt: &DataType,
    types: &Types,
    generics: &[(GenericReference, DataType)],
) -> bool {
    match dt {
        DataType::Primitive(Primitive::bool) => true,
        DataType::Enum(enm) => enm
            .variants
            .iter()
            .filter(|(_, variant)| !variant.skip)
            .all(|(_, variant)| match &variant.fields {
                Fields::Unit => true,
                Fields::Unnamed(fields) => {
                    let mut fields = fields.fields.iter().filter_map(|field| field.ty.as_ref());
                    fields
                        .next()
                        .is_some_and(|dt| map_key_is_finite(dt, types, generics))
                        && fields.next().is_none()
                }
                Fields::Named(_) => false,
            }),
        DataType::Struct(strct) => match &strct.fields {
            Fields::Unnamed(fields) => {
                let mut fields = fields.fields.iter().filter_map(|field| field.ty.as_ref());
                fields
                    .next()
                    .is_some_and(|dt| map_key_is_finite(dt, types, generics))
                    && fields.next().is_none()
            }
            _ => false,
        },
        DataType::Reference(Reference::Named(reference)) => types
            .get(reference)
            .and_then(|ndt| ndt.ty.as_ref())
            .is_some_and(|dt| {
                resolved_reference_generics(reference, generics)
                    .is_ok_and(|generics| map_key_is_finite(dt, types, &generics))
            }),
        DataType::Generic(generic) => {
            match generics.iter().find(|(candidate, _)| candidate == generic) {
                Some((_, dt)) if !matches!(dt, DataType::Generic(candidate) if candidate == generic) => {
                    map_key_is_finite(dt, types, generics)
                }
                _ => true,
            }
        }
        _ => false,
    }
}

fn map_key_datatype(
    s: &mut String,
    exporter: &Zod,
    types: &Types,
    dt: &DataType,
    location: Vec<Cow<'static, str>>,
    generics: &[(GenericReference, DataType)],
    type_render_stack: &mut TypeRenderStack,
) -> Result<(), Error> {
    match dt {
        DataType::Primitive(Primitive::str) => s.push_str("z.string()"),
        DataType::Primitive(Primitive::char) => {
            s.push_str("z.string().refine((value) => [...value].length === 1)")
        }
        DataType::Primitive(Primitive::bool) => s.push_str(r#"z.enum(["true", "false"])"#),
        DataType::Primitive(Primitive::i8) => s.push_str(
            r"z.string().regex(/^-?\d+$/).refine((value) => Number(value) >= -128 && Number(value) <= 127)",
        ),
        DataType::Primitive(Primitive::i16) => s.push_str(
            r"z.string().regex(/^-?\d+$/).refine((value) => Number(value) >= -32768 && Number(value) <= 32767)",
        ),
        DataType::Primitive(Primitive::i32) => s.push_str(
            r"z.string().regex(/^-?\d+$/).refine((value) => Number(value) >= -2147483648 && Number(value) <= 2147483647)",
        ),
        DataType::Primitive(Primitive::isize | Primitive::usize) => {
            return Err(Error::bigint_forbidden(location.join(".")));
        }
        DataType::Primitive(Primitive::u8) => s.push_str(
            r"z.string().regex(/^\d+$/).refine((value) => Number(value) <= 255)",
        ),
        DataType::Primitive(Primitive::u16) => s.push_str(
            r"z.string().regex(/^\d+$/).refine((value) => Number(value) <= 65535)",
        ),
        DataType::Primitive(Primitive::u32) => s.push_str(
            r"z.string().regex(/^\d+$/).refine((value) => Number(value) <= 4294967295)",
        ),
        DataType::Primitive(Primitive::f16 | Primitive::f32 | Primitive::f64) => {
            s.push_str(
                r"z.string().regex(/^-?(?:0|[1-9]\d*)(?:\.\d+)?(?:[eE][+-]?\d+)?$/).refine((value) => Number.isFinite(Number(value)))",
            )
        }
        DataType::Struct(strct) => {
            let Fields::Unnamed(fields) = &strct.fields else {
                return datatype(
                    s,
                    exporter,
                    types,
                    dt,
                    location,
                    generics,
                    false,
                    false,
                    type_render_stack,
                );
            };
            let mut fields = fields.fields.iter().filter_map(|field| field.ty.as_ref());
            let Some(field) = fields.next() else {
                return datatype(
                    s,
                    exporter,
                    types,
                    dt,
                    location,
                    generics,
                    false,
                    false,
                    type_render_stack,
                );
            };
            if fields.next().is_some() {
                return datatype(
                    s,
                    exporter,
                    types,
                    dt,
                    location,
                    generics,
                    false,
                    false,
                    type_render_stack,
                );
            }
            map_key_datatype(
                s,
                exporter,
                types,
                field,
                location,
                generics,
                type_render_stack,
            )?;
        }
        DataType::Enum(enm) => {
            let mut variants = Vec::new();
            for (name, variant) in enm.variants.iter().filter(|(_, variant)| !variant.skip) {
                let mut rendered = String::new();
                match &variant.fields {
                    Fields::Unit => write!(rendered, "z.literal(\"{}\")", escape_string(name))?,
                    Fields::Unnamed(fields) => {
                        let mut fields = fields.fields.iter().filter_map(|field| field.ty.as_ref());
                        let Some(field) = fields.next() else {
                            continue;
                        };
                        if fields.next().is_some() {
                            return datatype(
                                s,
                                exporter,
                                types,
                                dt,
                                location,
                                generics,
                                false,
                                false,
                                type_render_stack,
                            );
                        }
                        map_key_datatype(
                            &mut rendered,
                            exporter,
                            types,
                            field,
                            child_location(&location, name.to_string()),
                            generics,
                            type_render_stack,
                        )?;
                    }
                    Fields::Named(_) => {
                        return datatype(
                            s,
                            exporter,
                            types,
                            dt,
                            location,
                            generics,
                            false,
                            false,
                            type_render_stack,
                        );
                    }
                }
                variants.push(rendered);
            }
            match variants.as_slice() {
                [] => s.push_str("z.never()"),
                [variant] => s.push_str(variant),
                variants => write!(s, "z.union([{}])", variants.join(", "))?,
            }
        }
        DataType::Reference(Reference::Named(reference)) => {
            crate::references::track_nr(reference);
            let ty = named_reference_ty(types, reference)?;
            let reference_generics = resolved_reference_generics(reference, generics)?;
            map_key_datatype(
                s,
                exporter,
                types,
                ty,
                location,
                &reference_generics,
                type_render_stack,
            )?;
        }
        DataType::Generic(generic) => {
            if let Some((_, dt)) = generics.iter().find(|(candidate, _)| candidate == generic)
                && !matches!(dt, DataType::Generic(candidate) if candidate == generic)
            {
                map_key_datatype(
                    s,
                    exporter,
                    types,
                    dt,
                    location,
                    generics,
                    type_render_stack,
                )?;
            } else {
                s.push_str(&map_key_generic_name(generic.name()));
            }
        }
        _ => datatype(
            s,
            exporter,
            types,
            dt,
            location,
            generics,
            false,
            false,
            type_render_stack,
        )?,
    }
    Ok(())
}

fn resolved_reference_generics(
    reference: &NamedReference,
    outer_generics: &[(GenericReference, DataType)],
) -> Result<Vec<(GenericReference, DataType)>, Error> {
    named_reference_generics(reference).map(|generics| {
        generics
            .iter()
            .map(|(generic, dt)| {
                let mut dt = dt.clone();
                substitute_generics(&mut dt, outer_generics);
                (generic.clone(), dt)
            })
            .collect()
    })
}

fn substitute_generics(dt: &mut DataType, generics: &[(GenericReference, DataType)]) {
    match dt {
        DataType::Generic(generic) => {
            if let Some((_, replacement)) =
                generics.iter().find(|(candidate, _)| candidate == generic)
                && !matches!(replacement, DataType::Generic(candidate) if candidate == generic)
            {
                *dt = replacement.clone();
            }
        }
        DataType::List(list) => substitute_generics(&mut list.ty, generics),
        DataType::Map(map) => {
            substitute_generics(map.key_ty_mut(), generics);
            substitute_generics(map.value_ty_mut(), generics);
        }
        DataType::Nullable(inner) => substitute_generics(inner, generics),
        DataType::Struct(strct) => substitute_field_generics(&mut strct.fields, generics),
        DataType::Enum(enm) => enm
            .variants
            .iter_mut()
            .for_each(|(_, variant)| substitute_field_generics(&mut variant.fields, generics)),
        DataType::Tuple(tuple) => tuple
            .elements
            .iter_mut()
            .for_each(|dt| substitute_generics(dt, generics)),
        DataType::Intersection(types) => types
            .iter_mut()
            .for_each(|dt| substitute_generics(dt, generics)),
        DataType::Reference(Reference::Named(reference)) => match &mut reference.inner {
            NamedReferenceType::Inline { dt, .. } => substitute_generics(dt, generics),
            NamedReferenceType::Reference {
                generics: reference_generics,
                ..
            } => reference_generics
                .iter_mut()
                .for_each(|(_, dt)| substitute_generics(dt, generics)),
            NamedReferenceType::Recursive(_) => {}
        },
        DataType::Primitive(_) | DataType::Reference(Reference::Opaque(_)) => {}
    }
}

fn substitute_field_generics(fields: &mut Fields, generics: &[(GenericReference, DataType)]) {
    match fields {
        Fields::Unit => {}
        Fields::Unnamed(fields) => fields
            .fields
            .iter_mut()
            .filter_map(|field| field.ty.as_mut())
            .for_each(|dt| substitute_generics(dt, generics)),
        Fields::Named(fields) => fields
            .fields
            .iter_mut()
            .filter_map(|(_, field)| field.ty.as_mut())
            .for_each(|dt| substitute_generics(dt, generics)),
    }
}

fn tuple_dt(
    s: &mut String,
    exporter: &Zod,
    types: &Types,
    t: &Tuple,
    location: Vec<Cow<'static, str>>,
    generics: &[(GenericReference, DataType)],
    deferred: bool,
    type_render_stack: &mut TypeRenderStack,
) -> Result<(), Error> {
    match t.elements.as_slice() {
        [] => s.push_str("z.null()"),
        elements => {
            s.push_str("z.tuple([");
            for (i, dt) in elements.iter().enumerate() {
                if i != 0 {
                    s.push_str(", ");
                }
                datatype(
                    s,
                    exporter,
                    types,
                    dt,
                    child_location(&location, i.to_string()),
                    generics,
                    false,
                    deferred,
                    type_render_stack,
                )?;
            }
            s.push_str("])");
        }
    }

    Ok(())
}

fn struct_dt(
    s: &mut String,
    exporter: &Zod,
    types: &Types,
    st: &Struct,
    location: Vec<Cow<'static, str>>,
    generics: &[(GenericReference, DataType)],
    deferred: bool,
    type_render_stack: &mut TypeRenderStack,
) -> Result<(), Error> {
    match &st.fields {
        Fields::Unit => s.push_str("z.null()"),
        Fields::Unnamed(unnamed) => {
            let fields = unnamed
                .fields
                .iter()
                .filter_map(|field| field.ty.as_ref().map(|ty| (field, ty)))
                .collect::<Vec<_>>();

            match fields.as_slice() {
                [] => s.push_str("z.tuple([])"),
                [(field, ty)] if unnamed.fields.len() == 1 => {
                    datatype_with_inline_attr(
                        s,
                        exporter,
                        types,
                        ty,
                        location,
                        generics,
                        false,
                        deferred,
                        type_render_stack,
                    )?;
                }
                fields => {
                    // serde accepts sequences truncated anywhere inside the
                    // trailing run of defaulted (`optional`) elements; zod
                    // expresses that with trailing `.optional()` elements.
                    let mut optional_from = 0;
                    for (i, (field, _)) in fields.iter().enumerate() {
                        if !field.optional {
                            optional_from = i + 1;
                        }
                    }

                    s.push_str("z.tuple([");
                    for (i, (_field, ty)) in fields.iter().enumerate() {
                        if i != 0 {
                            s.push_str(", ");
                        }
                        datatype_with_inline_attr(
                            s,
                            exporter,
                            types,
                            ty,
                            child_location(&location, i.to_string()),
                            generics,
                            false,
                            deferred,
                            type_render_stack,
                        )?;
                        if i >= optional_from {
                            s.push_str(".optional()");
                        }
                    }
                    s.push_str("])");
                }
            }
        }
        Fields::Named(named) => {
            let all_fields = named
                .fields
                .iter()
                .filter_map(|(name, field)| field.ty.as_ref().map(|ty| (name, field, ty)))
                .collect::<Vec<_>>();

            if all_fields.is_empty() {
                // PATCH(1)：普通 struct 恒出 strict。
                s.push_str("z.strictObject({})");
                return Ok(());
            }

            let non_flattened = all_fields.iter().collect::<Vec<_>>();

            // PATCH(1)：普通 struct 恒出 strict（多余键拒绝写死，无 loose 变体）。
            let mut schema = String::from("z.strictObject({");
            for (name, field, ty) in &non_flattened {
                let key = sanitise_key(name.as_ref());
                // PATCH(3)：命名字段一律出 getter（`get x() {{ return … }}`）。
                // 取值表达式因此词法上位于 getter 体内——该位置对具名 schema 的
                // 引用必须裸引用（`z.lazy` 会让 TS 推导触发 TS7023 循环），故向下
                // 传 `deferred = true`。`optional` 仍落在返回表达式里。
                write!(schema, "\n\tget {key}() {{ return ")?;
                let mut value = String::new();
                datatype_with_inline_attr(
                    &mut value,
                    exporter,
                    types,
                    ty,
                    child_location(&location, name.to_string()),
                    generics,
                    false,
                    true,
                    type_render_stack,
                )?;
                if field.optional {
                    write!(schema, "{value}.optional(); }},")?;
                } else {
                    write!(schema, "{value}; }},")?;
                }
            }
            if !non_flattened.is_empty() {
                schema.push('\n');
            }
            schema.push_str("})");

            s.push_str(&schema);
        }
    }

    Ok(())
}

fn object_field_keys(
    dt: &DataType,
    types: &Types,
    generics: &[(GenericReference, DataType)],
    visiting: &mut HashSet<NamedReference>,
) -> Option<HashSet<String>> {
    match dt {
        DataType::Struct(strct) => match &strct.fields {
            Fields::Named(named) => Some(
                named
                    .fields
                    .iter()
                    .filter(|(_, field)| field.ty.is_some())
                    .map(|(name, _)| name.to_string())
                    .collect(),
            ),
            Fields::Unnamed(fields) => {
                let mut live = fields.fields.iter().filter_map(|field| field.ty.as_ref());
                let inner = live.next()?;
                if live.next().is_some() {
                    return None;
                }
                object_field_keys(inner, types, generics, visiting)
            }
            Fields::Unit => Some(HashSet::new()),
        },
        DataType::Intersection(parts) => {
            let mut keys = HashSet::new();
            for part in parts {
                keys.extend(object_field_keys(part, types, generics, visiting)?);
            }
            Some(keys)
        }
        DataType::Enum(enm) => {
            let mut keys = HashSet::new();
            for (_, variant) in enm.variants.iter().filter(|(_, variant)| !variant.skip) {
                keys.extend(object_field_keys_for_fields(
                    &variant.fields,
                    types,
                    generics,
                    visiting,
                )?);
            }
            Some(keys)
        }
        DataType::Reference(Reference::Named(reference)) => {
            if matches!(reference.inner, NamedReferenceType::Recursive(_))
                || !visiting.insert(reference.clone())
            {
                return None;
            }
            let result = (|| {
                let ty = named_reference_ty(types, reference).ok()?;
                let reference_generics = resolved_reference_generics(reference, generics).ok()?;
                object_field_keys(ty, types, &reference_generics, visiting)
            })();
            visiting.remove(reference);
            result
        }
        DataType::Generic(generic) => generics
            .iter()
            .find(|(candidate, _)| candidate == generic)
            .and_then(|(_, ty)| {
                (!matches!(ty, DataType::Generic(candidate) if candidate == generic))
                    .then(|| object_field_keys(ty, types, generics, visiting))
                    .flatten()
            }),
        _ => None,
    }
}

fn object_field_keys_for_fields(
    fields: &Fields,
    types: &Types,
    generics: &[(GenericReference, DataType)],
    visiting: &mut HashSet<NamedReference>,
) -> Option<HashSet<String>> {
    match fields {
        Fields::Named(named) => Some(
            named
                .fields
                .iter()
                .filter(|(_, field)| field.ty.is_some())
                .map(|(name, _)| name.to_string())
                .collect(),
        ),
        Fields::Unnamed(unnamed) => {
            let mut live = unnamed.fields.iter().filter_map(|field| field.ty.as_ref());
            let inner = live.next()?;
            if live.next().is_some() {
                return None;
            }
            object_field_keys(inner, types, generics, visiting)
        }
        Fields::Unit => Some(HashSet::new()),
    }
}

fn enum_dt(
    s: &mut String,
    exporter: &Zod,
    types: &Types,
    e: &Enum,
    location: Vec<Cow<'static, str>>,
    generics: &[(GenericReference, DataType)],
    deferred: bool,
    type_render_stack: &mut TypeRenderStack,
    allowed_object_keys: &[&str],
) -> Result<(), Error> {
    let optional_flatten_union = e.attributes.contains_key(OPTIONAL_FLATTEN_UNION_MARKER);
    let entries = e
        .variants
        .iter()
        .filter(|(_, variant)| !variant.skip)
        .map(|(name, variant)| {
            let keys = optional_flatten_union.then(|| {
                object_field_keys_for_fields(&variant.fields, types, generics, &mut HashSet::new())
            });
            (name, variant, keys.flatten())
        })
        .collect::<Vec<_>>();
    let all_optional_flatten_keys = entries
        .iter()
        .filter_map(|(_, _, keys)| keys.as_ref())
        .flat_map(|keys| keys.iter().cloned())
        .collect::<HashSet<_>>();

    let variants = entries
        .into_iter()
        .map(|(name, variant, keys)| -> Result<Option<String>, Error> {
            let rendered = enum_variant_dt(
                exporter,
                types,
                name.as_ref(),
                variant,
                child_location(&location, name.to_string()),
                generics,
                deferred,
                type_render_stack,
                allowed_object_keys,
            )?;
            let Some(mut rendered) = rendered else {
                return Ok(None);
            };

            if let Some(keys) = keys {
                let mut missing = all_optional_flatten_keys
                    .difference(&keys)
                    .map(String::as_str)
                    .collect::<Vec<_>>();
                missing.sort_unstable();
                if !missing.is_empty() {
                    rendered.push_str(".and(z.object({");
                    for key in missing {
                        write!(rendered, "\n\t{}: z.never().optional(),", sanitise_key(key))?;
                    }
                    rendered.push_str("\n}))");
                }
            }

            Ok(Some(rendered))
        })
        .collect::<Result<Vec<_>, _>>()?;

    let variants = variants.into_iter().flatten().collect::<Vec<_>>();

    if variants.is_empty() {
        s.push_str("z.never()");
        return Ok(());
    }

    let mut unique_variants = Vec::with_capacity(variants.len());
    for variant in variants {
        if !unique_variants.contains(&variant) {
            unique_variants.push(variant);
        }
    }
    let variants = unique_variants;

    if variants.len() == 1 {
        s.push_str(&variants[0]);
    } else {
        write!(s, "z.union([{}])", variants.join(", "))?;
    }

    Ok(())
}

fn enum_variant_dt(
    exporter: &Zod,
    types: &Types,
    name: &str,
    variant: &specta::datatype::Variant,
    location: Vec<Cow<'static, str>>,
    generics: &[(GenericReference, DataType)],
    deferred: bool,
    type_render_stack: &mut TypeRenderStack,
    allowed_object_keys: &[&str],
) -> Result<Option<String>, Error> {
    match &variant.fields {
        Fields::Unit => Ok(Some(format!("z.literal(\"{}\")", escape_string(name)))),
        Fields::Named(named) => {
            // PATCH(1)：变体对象面同样恒出 strict（原按 `strict_object` 标记二选一）。
            let mut schema = String::from("z.strictObject({");
            let mut has_field = false;

            for (field_name, field) in &named.fields {
                let Some(ty) = field.ty.as_ref() else {
                    continue;
                };

                has_field = true;
                let mut value = String::new();
                // PATCH(3)：变体命名字段同样出 getter，取值表达式传 `deferred = true`
                // （理由同 `struct_dt` 的命名分支）。
                datatype_with_inline_attr(
                    &mut value,
                    exporter,
                    types,
                    ty,
                    child_location(&location, field_name.to_string()),
                    generics,
                    false,
                    true,
                    type_render_stack,
                )?;

                let key = sanitise_key(field_name);
                if field.optional {
                    write!(schema, "\n\tget {key}() {{ return {value}.optional(); }},")?;
                } else {
                    write!(schema, "\n\tget {key}() {{ return {value}; }},")?;
                }
            }

            for field_name in allowed_object_keys {
                if named
                    .fields
                    .iter()
                    .any(|(name, field)| field.ty.is_some() && name.as_ref() == *field_name)
                {
                    continue;
                }
                has_field = true;
                let key = sanitise_key(field_name);
                write!(schema, "\n\t{key}: z.unknown().optional(),")?;
            }

            if has_field {
                schema.push('\n');
            }
            schema.push_str("})");

            Ok(Some(schema))
        }
        Fields::Unnamed(unnamed) => {
            let fields = unnamed
                .fields
                .iter()
                .filter_map(|field| field.ty.as_ref().map(|ty| (field, ty)))
                .collect::<Vec<_>>();

            Ok(match fields.as_slice() {
                [] => {
                    if unnamed.fields.is_empty() {
                        Some("z.tuple([])".to_string())
                    } else {
                        None
                    }
                }
                [(field, ty)] if unnamed.fields.len() == 1 => {
                    let mut out = String::new();
                    datatype_with_inline_attr(
                        &mut out,
                        exporter,
                        types,
                        ty,
                        location,
                        generics,
                        false,
                        deferred,
                        type_render_stack,
                    )?;
                    Some(out)
                }
                fields => {
                    let mut optional_from = 0;
                    for (i, (field, _)) in fields.iter().enumerate() {
                        if !field.optional {
                            optional_from = i + 1;
                        }
                    }

                    let mut out = String::from("z.tuple([");
                    for (i, (_field, ty)) in fields.iter().enumerate() {
                        if i != 0 {
                            out.push_str(", ");
                        }
                        let mut item = String::new();
                        datatype_with_inline_attr(
                            &mut item,
                            exporter,
                            types,
                            ty,
                            child_location(&location, i.to_string()),
                            generics,
                            false,
                            deferred,
                            type_render_stack,
                        )?;
                        out.push_str(&item);
                        if i >= optional_from {
                            out.push_str(".optional()");
                        }
                    }
                    out.push_str("])");
                    Some(out)
                }
            })
        }
    }
}

fn reference_dt(
    s: &mut String,
    exporter: &Zod,
    types: &Types,
    r: &Reference,
    location: Vec<Cow<'static, str>>,
    generics: &[(GenericReference, DataType)],
    deferred: bool,
    type_render_stack: &mut TypeRenderStack,
) -> Result<(), Error> {
    match r {
        Reference::Named(r) => reference_named_dt(
            s,
            exporter,
            types,
            r,
            location,
            generics,
            deferred,
            type_render_stack,
        ),
        Reference::Opaque(r) => reference_opaque_dt(s, r),
    }
}

fn generic_dt(s: &mut String, g: &GenericReference) {
    s.push_str(g.name());
}

fn reference_opaque_dt(s: &mut String, r: &OpaqueReference) -> Result<(), Error> {
    if let Some(def) = r.downcast_ref::<opaque::Define>() {
        s.push_str(&def.0);
        return Ok(());
    }
    if r.downcast_ref::<opaque::Any>().is_some() {
        s.push_str("z.any()");
        return Ok(());
    }
    if r.downcast_ref::<opaque::Unknown>().is_some() {
        s.push_str("z.unknown()");
        return Ok(());
    }
    if r.downcast_ref::<opaque::Never>().is_some() {
        s.push_str("z.never()");
        return Ok(());
    }

    Err(Error::unsupported_opaque_reference(r.clone()))
}

fn reference_named_dt(
    s: &mut String,
    exporter: &Zod,
    types: &Types,
    r: &NamedReference,
    location: Vec<Cow<'static, str>>,
    generics: &[(GenericReference, DataType)],
    deferred: bool,
    type_render_stack: &mut TypeRenderStack,
) -> Result<(), Error> {
    let ndt = types
        .get(r)
        .ok_or_else(|| Error::dangling_named_reference(format!("{r:?}")))?;

    if matches!(r.inner, NamedReferenceType::Inline { .. }) {
        let ty = named_reference_ty(types, r)?;
        let reference_generics = named_reference_generics(r)?;
        return datatype(
            s,
            exporter,
            types,
            ty,
            location,
            reference_generics,
            false,
            deferred,
            type_render_stack,
        );
    }

    crate::references::track_nr(r);

    let schema_name = match exporter.layout {
        Layout::Namespaces => {
            let mut name = String::from("$s$");
            for segment in crate::zod::namespace_module_path(&ndt.module_path)
                .split('.')
                .filter(|segment| !segment.is_empty())
            {
                name.push('.');
                name.push_str(segment);
            }
            name.push('.');
            name.push_str(&ndt.name);
            name.push_str("Schema");
            name
        }
        Layout::FlatFile | Layout::ModulePrefixedName => {
            format!("{}Schema", exported_type_name(exporter, ndt))
        }
        Layout::Files => {
            let current_module_path = crate::references::current_module_path().unwrap_or_default();
            let base = format!("{}Schema", ndt.name);
            if ndt.module_path == current_module_path {
                base
            } else {
                format!("{}.{}", crate::zod::module_alias(&ndt.module_path), base)
            }
        }
    };

    let mut reference_expr = schema_name;
    let reference_generics = match &r.inner {
        NamedReferenceType::Recursive(recursive) => recursive.generics(),
        _ => named_reference_generics(r)?,
    };
    if !reference_generics.is_empty() {
        let scoped_generics = generics
            .iter()
            .filter(|(parent_generic, _)| {
                !reference_generics
                    .iter()
                    .any(|(child_generic, _)| child_generic == parent_generic)
            })
            .cloned()
            .collect::<Vec<_>>();

        let map_key_generics = named_map_key_parameters(ndt, types);
        let mut schema_arguments = Vec::new();
        for (generic, v) in reference_generics {
            let mut generic_schema = String::new();
            datatype(
                &mut generic_schema,
                exporter,
                types,
                v,
                vec![],
                &scoped_generics,
                false,
                deferred,
                type_render_stack,
            )?;
            schema_arguments.push(generic_schema);
            if map_key_generics.contains(generic.name().as_ref()) {
                let mut resolved_key = v.clone();
                substitute_generics(&mut resolved_key, &scoped_generics);
                map_keys::validate_map_key(
                    &resolved_key,
                    types,
                    format!("{}.<generic {} key>", ndt.name, generic.name()),
                )?;
                let mut key_schema = String::new();
                map_key_datatype(
                    &mut key_schema,
                    exporter,
                    types,
                    v,
                    vec![],
                    &scoped_generics,
                    type_render_stack,
                )?;
                schema_arguments.push(key_schema);
            }
        }
        reference_expr.push('(');
        reference_expr.push_str(&schema_arguments.join(", "));
        reference_expr.push(')');
    }

    // Declarations are sorted, so this reference may target a schema whose
    // initializer has not run yet. Laziness also handles recursive references.
    //
    // PATCH(3)：`deferred` 表示该引用词法上位于字段 getter 体内——那种位置已经
    // 天然惰性（shape 首次读取时才求值），必须裸引用：`z.lazy` 会让 TypeScript
    // 推导出循环返回类型（TS7023）并连带整棵 `z.infer` 塌成 `any`。
    if deferred {
        write!(s, "{reference_expr}")?;
    } else {
        write!(s, "z.lazy(() => {reference_expr})")?;
    }

    Ok(())
}

pub(crate) fn exported_type_name(exporter: &Zod, ndt: &NamedDataType) -> Cow<'static, str> {
    match exporter.layout {
        Layout::Namespaces | Layout::FlatFile | Layout::Files => ndt.name.clone(),
        Layout::ModulePrefixedName => {
            let mut s = ndt.module_path.split("::").collect::<Vec<_>>().join("_");
            if !s.is_empty() {
                s.push('_');
            }
            s.push_str(&ndt.name);
            Cow::Owned(s)
        }
    }
}

fn validate_type_name(name: &str, path: String) -> Result<(), Error> {
    if RESERVED_TYPE_NAMES.contains(&name) {
        return Err(Error::forbidden_name(path, name.to_string()));
    }

    let mut chars = name.chars();
    let Some(first) = chars.next() else {
        return Err(Error::invalid_name(path, name.to_string()));
    };

    if !(first.is_alphabetic() || first == '_') {
        return Err(Error::invalid_name(path, name.to_string()));
    }
    if chars.any(|ch| !(ch.is_alphanumeric() || ch == '_')) {
        return Err(Error::invalid_name(path, name.to_string()));
    }

    Ok(())
}

fn sanitise_key(field_name: &str) -> String {
    if field_name == "__proto__" {
        // `__proto__: value` is prototype-setter syntax in JavaScript object
        // literals, so it must be emitted as a computed own property.
        "[\"__proto__\"]".to_string()
    } else if is_identifier(field_name) {
        field_name.to_string()
    } else {
        format!("\"{}\"", escape_string(field_name))
    }
}

fn child_location(
    location: &[Cow<'static, str>],
    child: impl Into<Cow<'static, str>>,
) -> Vec<Cow<'static, str>> {
    let mut location = location.to_vec();
    location.push(child.into());
    location
}

fn is_identifier(name: &str) -> bool {
    let mut chars = name.chars();
    let Some(first) = chars.next() else {
        return false;
    };

    (first.is_ascii_alphabetic() || first == '_' || first == '$')
        && chars.all(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == '$')
}

fn escape_string(value: &str) -> Cow<'_, str> {
    if !value.chars().any(|ch| {
        ch == '"' || ch == '\\' || ch == '\u{2028}' || ch == '\u{2029}' || ch.is_control()
    }) {
        return Cow::Borrowed(value);
    }

    let mut escaped = String::with_capacity(value.len());
    for ch in value.chars() {
        match ch {
            '"' => escaped.push_str(r#"\""#),
            '\\' => escaped.push_str(r#"\\"#),
            '\n' => escaped.push_str(r#"\n"#),
            '\r' => escaped.push_str(r#"\r"#),
            '\t' => escaped.push_str(r#"\t"#),
            '\u{2028}' => escaped.push_str(r#"\u2028"#),
            '\u{2029}' => escaped.push_str(r#"\u2029"#),
            ch if ch.is_control() => push_unicode_escape(&mut escaped, ch),
            _ => escaped.push(ch),
        }
    }

    Cow::Owned(escaped)
}

fn push_unicode_escape(s: &mut String, ch: char) {
    const HEX: &[u8; 16] = b"0123456789ABCDEF";
    let value = ch as u32;

    s.push_str(r#"\u"#);
    s.push(HEX[((value >> 12) & 0xF) as usize] as char);
    s.push(HEX[((value >> 8) & 0xF) as usize] as char);
    s.push(HEX[((value >> 4) & 0xF) as usize] as char);
    s.push(HEX[(value & 0xF) as usize] as char);
}
