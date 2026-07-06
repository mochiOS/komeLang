use crate::position::{position_to_byte_offset, span_to_range};
use kome_ast::declarations::Module;
use kome_semantics::{resolver::ScopeBuilder, scope::SourceId};
use komec::stdlib::StandardLibrary;
use tower_lsp::lsp_types::{Location, Position, Url};

const DOCUMENT_SOURCE: SourceId = 0;

pub fn definition_at(
    document_uri: &Url,
    document_source: &str,
    position: Position,
) -> Option<Location> {
    let application = kome_parser::parse(document_source).ok()?;

    let standard_modules = StandardLibrary::discover()
        .ok()
        .and_then(|library| library.modules_for(&application).ok())
        .unwrap_or_default();

    let mut source_modules: Vec<(SourceId, &Module)> =
        Vec::with_capacity(standard_modules.len() + 1);

    for (index, loaded) in standard_modules.iter().enumerate() {
        source_modules.push((index + 1, &loaded.module));
    }

    source_modules.push((DOCUMENT_SOURCE, &application));

    let resolution = ScopeBuilder::resolve_sources(&source_modules);

    let byte_offset = position_to_byte_offset(document_source, position)?;

    let reference = resolution
        .references
        .iter()
        .filter(|reference| reference.source == Option::from(DOCUMENT_SOURCE))
        .filter(|reference| {
            reference.span.start <= byte_offset && byte_offset <= reference.span.end
        })
        .min_by_key(|reference| reference.span.end - reference.span.start)?;

    let symbol_id = reference.resolved_to?;

    let source_id = resolution
        .symbol_sources
        .get(symbol_id)
        .copied()
        .flatten()?;

    let symbol = resolution.symbols.get(symbol_id)?;

    let definition_span = symbol.definition_span()?;

    if source_id == DOCUMENT_SOURCE {
        return Some(Location {
            uri: document_uri.clone(),
            range: span_to_range(document_source, definition_span),
        });
    }

    let loaded = standard_modules.get(source_id - 1)?;

    let uri = Url::from_file_path(&loaded.path).ok()?;

    Some(Location {
        uri,
        range: span_to_range(&loaded.source, definition_span),
    })
}
