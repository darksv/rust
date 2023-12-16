#![allow(warnings)]

use std::path::Path;

use thin_vec::{thin_vec, ThinVec};

use rustc_ast::{ast, Defaultness, DUMMY_NODE_ID, Extern, FnDecl, FnHeader,
                FnRetTy, FnSig, ForeignItemKind, Generics, Item, ItemKind, MutTy, StrLit,
                token, Ty, TyKind, Unsafe, Visibility, VisibilityKind};
use rustc_ast::attr;
use rustc_ast::ptr::P;
use rustc_ast::token::{LitKind};

use rustc_session::Session;
use rustc_span::{DUMMY_SP, SourceFile, Span, Symbol};
use rustc_span::symbol::Ident;

fn clang_to_rustc(source_file: &SourceFile, range: clang::source::SourceRange) -> Span {
    let file_pos = source_file.start_pos;
    Span::with_root_ctxt(
        file_pos + rustc_span::BytePos(range.get_start().get_file_location().offset),
        file_pos + rustc_span::BytePos(range.get_end().get_file_location().offset),
    )
}

fn make_ty(source_file: &SourceFile, field: &clang::Entity) -> Ty {
    let ty = field.get_type().unwrap();
    let kind = match ty.get_kind() {
        clang::TypeKind::Void => TyKind::Tup(thin_vec![]),
        clang::TypeKind::Bool => TyKind::Path(None, ast::Path::from_ident(Ident::from_str("bool"))),
        clang::TypeKind::Int => TyKind::Path(None, ast::Path::from_ident(Ident::from_str("i32"))),
        clang::TypeKind::UInt => TyKind::Path(None, ast::Path::from_ident(Ident::from_str("u32"))),
        clang::TypeKind::Float => TyKind::Path(None, ast::Path::from_ident(Ident::from_str("f32"))),
        clang::TypeKind::Double => TyKind::Path(None, ast::Path::from_ident(Ident::from_str("f64"))),
        clang::TypeKind::Pointer => {
            let pointee = ty.get_pointee_type().unwrap();
            let name = pointee.get_typedef_name().unwrap_or_else(|| pointee.get_display_name());

            let name = name.strip_prefix("struct ").unwrap_or(&name);
            TyKind::Ptr(MutTy {
                ty: P(ast::Ty {
                    id: DUMMY_NODE_ID,
                    kind: TyKind::Path(None, ast::Path::from_ident(Ident::from_str(name))),
                    span: Default::default(),
                    tokens: None,
                }),
                mutbl: if ty.is_const_qualified() { ast::Mutability::Not } else { ast::Mutability::Mut },
            })
        }
        _ => unimplemented!(),
    };

    Ty {
        id: DUMMY_NODE_ID,
        kind: kind,
        span: clang_to_rustc(source_file, field.get_range().unwrap()),
        tokens: None,
    }
}

pub(crate) fn parse(path: &Path, source_file: &SourceFile, sess: &Session) -> ThinVec<P<Item>> {
    let mut items = ThinVec::new();
    let clang = clang::Clang::new().unwrap();
    let index = clang::Index::new(&clang, false, true);
    let tu = index.parser(&path).parse().unwrap();

    for entity in tu.get_entity().get_children() {
        match entity.get_kind() {
            clang::EntityKind::VarDecl if !entity.is_mutable() => {
                let item = Item {
                    ident: Ident::new(
                        Symbol::intern(&entity.get_name().unwrap()),
                        clang_to_rustc(&source_file, entity.get_name_ranges()[0]),
                    ),
                    attrs: thin_vec![],
                    id: DUMMY_NODE_ID,
                    kind: ItemKind::Const(Box::new(
                        ast::ConstItem {
                            defaultness: Defaultness::Final,
                            generics: Default::default(),
                            ty: P(make_ty(source_file, &entity)),
                            expr: Some(P(ast::Expr {
                                id: DUMMY_NODE_ID,
                                kind: ast::ExprKind::Lit(token::Lit::new(
                                    LitKind::Integer,
                                    Symbol::intern("123"),
                                    None,
                                )),
                                span: Default::default(),
                                attrs: Default::default(),
                                tokens: None,
                            })),
                        }
                    )),
                    vis: Visibility {
                        kind: rustc_ast::VisibilityKind::Public,
                        span: DUMMY_SP,
                        tokens: None,
                    },
                    span: entity.get_range().map(|it| clang_to_rustc(&source_file, it)).unwrap_or(DUMMY_SP),
                    tokens: None,
                };
                items.push(P(item));
            }
            clang::EntityKind::FunctionDecl => {
                let inputs: ThinVec<_> = entity.get_arguments().iter().flatten().map(|it| {
                    ast::Param {
                        attrs: Default::default(),
                        ty: P(make_ty(source_file, it)),
                        pat: P(ast::Pat {
                            id: DUMMY_NODE_ID,
                            kind: ast::PatKind::Ident(ast::BindingAnnotation::NONE, Ident::from_str(it.get_name().as_deref().unwrap()), None),
                            span: Default::default(),
                            tokens: None,
                        }),
                        id: DUMMY_NODE_ID,
                        span: Default::default(),
                        is_placeholder: false,
                    }
                }).collect();

                let fn_item = Item {
                    ident: Ident::new(
                        Symbol::intern(&entity.get_name().unwrap()),
                        clang_to_rustc(&source_file, entity.get_name_ranges()[0]),
                    ),
                    attrs: thin_vec![
                        // attr::mk_attr_word(
                        //     &sess.parse_sess.attr_id_generator,
                        //     AttrStyle::Outer,
                        //     Symbol::intern("no_mangle"),
                        //     DUMMY_SP
                        // )
                    ],
                    id: DUMMY_NODE_ID,
                    kind: ForeignItemKind::Fn(Box::new(
                        ast::Fn {
                            defaultness: Defaultness::Final,
                            generics: Generics::default(),
                            sig: FnSig {
                                header: FnHeader {
                                    unsafety: ast::Unsafe::No,
                                    coroutine_kind: None,
                                    constness: ast::Const::No,
                                    ext: Extern::None,
                                },
                                decl: P(FnDecl {
                                    inputs,
                                    output: FnRetTy::Default(DUMMY_SP),//FnRetTy::Ty(make_ty(entity.get_result_type())),
                                }),
                                span: clang_to_rustc(&source_file, entity.get_range().unwrap()),
                            },
                            body: None,
                        }
                    )),
                    vis: Visibility {
                        kind: VisibilityKind::Public,
                        span: DUMMY_SP,
                        tokens: None,
                    },
                    span: entity.get_range().map(|it| clang_to_rustc(&source_file, it)).unwrap_or(DUMMY_SP),
                    tokens: None,
                };

                let item = Item {
                    ident: Ident::empty(),
                    attrs: thin_vec![],
                    id: DUMMY_NODE_ID,
                    span: Span::with_root_ctxt(source_file.start_pos, source_file.end_position()),
                    kind: ItemKind::ForeignMod(ast::ForeignMod {
                        unsafety: Unsafe::No,
                        abi: Some(StrLit {
                            symbol: Symbol::intern("C"),
                            suffix: None,
                            symbol_unescaped: Symbol::intern("C"),
                            style: ast::StrStyle::Cooked,
                            span: Default::default(),
                        }),
                        items: thin_vec![
                            P(fn_item)
                        ],
                    }),
                    vis: Visibility {
                        kind: ast::VisibilityKind::Inherited,
                        span: DUMMY_SP,
                        tokens: None,
                    },
                    tokens: None,
                };

                items.push(P(item));
            }
            clang::EntityKind::StructDecl => {
                let mut fields = ThinVec::new();
                for field in entity.get_children() {
                    fields.push(rustc_ast::FieldDef {
                        attrs: ThinVec::new(),
                        id: DUMMY_NODE_ID,
                        span: field.get_range().map(|it| clang_to_rustc(&source_file, it)).unwrap_or(DUMMY_SP),
                        vis: Visibility { kind: rustc_ast::VisibilityKind::Public, span: DUMMY_SP, tokens: None },
                        ident: Some(Ident::new(
                            Symbol::intern(&field.get_name().unwrap()),
                            clang_to_rustc(&source_file, field.get_name_ranges()[0]),
                        )),
                        ty: P(make_ty(source_file, &field)),
                        is_placeholder: false,
                    });
                }

                let item = Item {
                    ident: Ident::new(
                        Symbol::intern(&entity.get_name().unwrap()),
                        clang_to_rustc(&source_file, entity.get_name_ranges()[0]),
                    ),
                    attrs: thin_vec![
                        attr::mk_attr_nested_word(
                            &sess.parse_sess.attr_id_generator,
                            rustc_ast::AttrStyle::Outer,
                            Symbol::intern("repr"),
                            Symbol::intern("C"),
                            DUMMY_SP
                        )
                    ],
                    id: DUMMY_NODE_ID,
                    kind: ItemKind::Struct(
                        rustc_ast::VariantData::Struct(fields, false),
                        rustc_ast::Generics::default(),
                    ),
                    vis: Visibility {
                        kind: rustc_ast::VisibilityKind::Public,
                        span: DUMMY_SP,
                        tokens: None,
                    },
                    span: entity.get_range().map(|it| clang_to_rustc(&source_file, it)).unwrap_or(DUMMY_SP),
                    tokens: None,
                };
                items.push(P(item));
            }
            _ => (),
        }
    }

    items
}