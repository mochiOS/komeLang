use crate::position::{position_to_byte_offset, span_to_range};
use kome_ast::declarations::Module;
use kome_semantics::{
    resolver::ScopeBuilder,
    scope::{Reference, SourceId},
};
use komec::stdlib::StandardLibrary;
use tower_lsp::lsp_types::{Location, Position, Url};

const DOCUMENT_SOURCE: SourceId = 0;

pub fn definition_at(
    document_uri: &Url,
    document_source: &str,
    position: Position,
) -> Option<Location> {
    let application = kome_parser::parse(document_source).ok()?;

    let byte_offset = position_to_byte_offset(document_source, position)?;

    /*
     * Resolve the application separately first.
     * This guarantees that the reference under the cursor is selected
     * from the currently opened document rather than from a standard
     * library file with overlapping byte spans.
     */
    let application_resolution = ScopeBuilder::resolve(&application);

    let target_reference = reference_at_offset(&application_resolution.references, byte_offset)?;

    /*
     * A locally resolved symbol can be returned without loading the
     * standard library.
     */
    if let Some(symbol_id) = target_reference.resolved_to {
        let symbol = application_resolution.symbols.get(symbol_id)?;

        let definition_span = symbol.definition_span()?;

        return Some(Location {
            uri: document_uri.clone(),
            range: span_to_range(document_source, definition_span),
        });
    }

    /*
     * Unresolved local references may come from imported modules.
     */
    let standard_library = StandardLibrary::discover().ok()?;

    let standard_modules = standard_library.modules_for(&application).ok()?;

    let mut source_modules: Vec<(SourceId, &Module)> =
        Vec::with_capacity(standard_modules.len() + 1);

    for (index, loaded) in standard_modules.iter().enumerate() {
        source_modules.push((index + 1, &loaded.module));
    }

    source_modules.push((DOCUMENT_SOURCE, &application));

    let resolution = ScopeBuilder::resolve_sources(&source_modules);

    /*
     * Match the same application reference in the multi-source
     * resolution result.
     */
    let resolved_reference = resolution
        .references
        .iter()
        .find(|reference| {
            reference.source == Option::from(DOCUMENT_SOURCE)
                && reference.span == target_reference.span
                && reference.name == target_reference.name
        })
        /*
         * Keep a fallback while SourceId propagation is still
         * being stabilized.
         */
        .or_else(|| {
            resolution.references.iter().find(|reference| {
                reference.span == target_reference.span && reference.name == target_reference.name
            })
        })?;

    let symbol_id = resolved_reference.resolved_to?;

    let symbol = resolution.symbols.get(symbol_id)?;

    let definition_span = symbol.definition_span()?;

    let source_id = resolution
        .symbol_sources
        .get(symbol_id)
        .copied()
        .flatten()?;

    if source_id == DOCUMENT_SOURCE {
        return Some(Location {
            uri: document_uri.clone(),
            range: span_to_range(document_source, definition_span),
        });
    }

    let loaded = standard_modules.get(source_id - 1)?;

    let definition_path = loaded.path.canonicalize().ok()?;

    let uri = Url::from_file_path(definition_path).ok()?;

    Some(Location {
        uri,
        range: span_to_range(&loaded.source, definition_span),
    })
}

fn reference_at_offset<'a>(
    references: &'a [Reference],
    byte_offset: usize,
) -> Option<&'a Reference> {
    references
        .iter()
        .filter(|reference| {
            span_contains_offset(reference, byte_offset)
                || byte_offset
                    .checked_sub(1)
                    .is_some_and(|previous| span_contains_offset(reference, previous))
        })
        .min_by_key(|reference| reference.span.end.saturating_sub(reference.span.start))
}

fn span_contains_offset(reference: &Reference, byte_offset: usize) -> bool {
    reference.span.start <= byte_offset && byte_offset < reference.span.end
}
