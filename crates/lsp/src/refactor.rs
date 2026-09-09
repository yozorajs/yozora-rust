use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::path::{Component, Path, PathBuf};

use yozora_ast::Node;
use yozora_ast_util::calc_heading_identifiers;
use yozora_parser::YozoraParser;

use crate::analysis::{heading_selection_range, nodes, plain_text};
use crate::cancellation::Cancellation;
use crate::document::{Document, Snapshot};
use crate::files::{self, FileScope, SourceBuffer, Workspace};
use crate::protocol::{FileRename, Position, PrepareRenameResult, Range, ResponseError, TextEdit};
use crate::resource_edit::{self, failed, Budget, HeadingChange, Replacement};

pub struct DocumentEdits {
    pub uri: String,
    pub version: Option<i32>,
    pub edits: Vec<TextEdit>,
}

/// The worker owns these document snapshots. Refactors return edits and keep
/// source text unchanged; the client owns edit application and filesystem moves.
pub struct Context<'a> {
    pub workspace: &'a Workspace,
    pub documents: &'a mut HashMap<String, Document>,
    pub parser: &'a YozoraParser,
    pub cancellation: &'a Cancellation,
    pub used_buffers: &'a mut HashSet<String>,
    pub heading_id_prefix: &'a str,
}

struct HeadingSubject {
    node_start: Position,
    range: Range,
    placeholder: String,
    empty: bool,
}

fn subject(snapshot: &Snapshot<'_>, position: Position) -> Option<HeadingSubject> {
    nodes(&snapshot.root.children)
        .filter_map(|node| {
            let Node::Heading(heading) = node else {
                return None;
            };
            let node_range = Range::from(heading.position.as_ref()?);
            let empty = heading.children.is_empty();
            let range = if empty {
                let start = snapshot
                    .lines
                    .byte_offset(snapshot.text, node_range.start)
                    .ok()?;
                let end = snapshot
                    .lines
                    .byte_offset(snapshot.text, node_range.end)
                    .ok()?;
                let text = snapshot.text.get(start..end)?;
                let marker = text.find('#')?;
                let point = snapshot
                    .lines
                    .position(snapshot.text, start + marker + heading.depth as usize)
                    .ok()?;
                Range {
                    start: point,
                    end: point,
                }
            } else {
                heading_selection_range(heading, node_range)
            };
            (range.start <= position && position <= range.end).then(|| HeadingSubject {
                node_start: node_range.start,
                range,
                placeholder: plain_text(&heading.children),
                empty,
            })
        })
        .last()
}

pub fn prepare(snapshot: &Snapshot<'_>, position: Position) -> Option<PrepareRenameResult> {
    let subject = subject(snapshot, position)?;
    Some(PrepareRenameResult {
        range: subject.range,
        placeholder: subject.placeholder,
    })
}

struct HeadingPlan {
    uri: String,
    path: Option<PathBuf>,
    change: HeadingChange,
    identifiers: BTreeMap<String, String>,
    activated: BTreeSet<String>,
}

struct FileMove {
    old: PathBuf,
    new: PathBuf,
    new_uri: String,
}

enum Plan {
    Heading(HeadingPlan),
    Files(Vec<FileMove>),
}

impl Plan {
    fn heading(&self, uri: &str) -> Option<&HeadingChange> {
        match self {
            Self::Heading(plan) if plan.uri == uri => Some(&plan.change),
            _ => None,
        }
    }

    fn destination(
        &self,
        source: &SourceBuffer,
        url: &str,
        scope: &FileScope,
    ) -> Result<Option<String>, ResponseError> {
        let Some(files::Target::Local { file, fragment, .. }) =
            files::resolve(&source.uri, url, scope)
        else {
            return Ok(None);
        };
        match self {
            Self::Heading(plan) => {
                let points_here = match file {
                    Some(file) => plan.path.as_ref() == Some(&file.path),
                    None => {
                        plan.uri == source.uri || (plan.path.is_some() && plan.path == source.path)
                    }
                };
                let Some(fragment) = fragment.filter(|fragment| !fragment.is_empty()) else {
                    return Ok(None);
                };
                if !points_here {
                    return Ok(None);
                }
                if plan.activated.contains(&fragment) {
                    return Err(failed(
                        "rename would activate a previously missing heading anchor",
                    ));
                }
                Ok(plan.identifiers.get(&fragment).map(|identifier| {
                    let resource = url
                        .split_once('#')
                        .expect("a resolved fragment has a delimiter")
                        .0;
                    format!("{resource}#{}", files::encode_component(identifier))
                }))
            }
            Self::Files(moves) => {
                let Some(file) = file else { return Ok(None) };
                let target = moves
                    .iter()
                    .find(|item| item.old == file.path)
                    .map_or(&file.path, |item| &item.new);
                let new_source = moves
                    .iter()
                    .find(|item| source.path.as_ref() == Some(&item.old));
                let new_source_uri =
                    new_source.map_or(source.uri.as_str(), |item| item.new_uri.as_str());
                if matches!(files::resolve(new_source_uri, url, scope), Some(files::Target::Local { file: Some(file), .. }) if &file.path == target)
                {
                    return Ok(None);
                }
                let end = url.find(['?', '#']).unwrap_or(url.len());
                let resource = &url[..end];
                let path = if resource
                    .split_once(':')
                    .is_some_and(|(scheme, _)| scheme.eq_ignore_ascii_case("file"))
                {
                    files::path_uri(target)
                        .ok_or_else(|| failed("renamed path cannot form a file URI"))?
                } else if resource.starts_with('/') {
                    files::path_uri(target)
                        .and_then(|uri| uri.strip_prefix("file://").map(str::to_string))
                        .ok_or_else(|| failed("renamed path cannot form an absolute URI path"))?
                } else {
                    let source = files::file_uri_path(new_source_uri)
                        .ok_or_else(|| failed("relative file links require a local source URI"))?;
                    relative_uri(
                        source
                            .parent()
                            .ok_or_else(|| failed("source has no parent directory"))?,
                        target,
                    )?
                };
                Ok(Some(format!("{path}{}", &url[end..])))
            }
        }
    }
}

impl Context<'_> {
    pub fn heading(
        &mut self,
        uri: &str,
        position: Position,
        name: &str,
    ) -> Result<Vec<DocumentEdits>, ResponseError> {
        self.cancellation.check()?;
        if name.is_empty()
            || name.trim() != name
            || name.chars().count() > 1_000
            || name.chars().any(char::is_control)
        {
            return Err(ResponseError::invalid_params("heading names must be nonempty single-line text of at most 1000 characters, without surrounding whitespace"));
        }
        self.used_buffers.insert(uri.to_string());
        let document = self
            .documents
            .get_mut(uri)
            .ok_or_else(|| failed("heading document is not open"))?;
        let snapshot = document.snapshot(self.parser, position)?;
        self.cancellation.check()?;
        let subject =
            subject(&snapshot, position).ok_or_else(|| failed("position is not heading text"))?;
        if snapshot
            .root
            .children
            .iter()
            .filter(|node| matches!(node, Node::Heading(_)))
            .take(20_001)
            .count()
            > 20_000
        {
            return Err(ResponseError::new(
                -32000,
                "heading rename exceeds the heading count limit",
            ));
        }
        if name == subject.placeholder {
            return Ok(Vec::new());
        }
        let mut text = if subject.empty {
            " ".to_string()
        } else {
            String::new()
        };
        for character in name.chars() {
            if character.is_ascii_punctuation() {
                text.push('\\');
            }
            text.push(character);
        }
        let change = HeadingChange {
            node_start: subject.node_start,
            replacement: Replacement {
                range: snapshot
                    .lines
                    .byte_offset(snapshot.text, subject.range.start)?
                    ..snapshot
                        .lines
                        .byte_offset(snapshot.text, subject.range.end)?,
                text,
            },
            name: name.to_string(),
        };
        let mut budget = Budget::default();
        let preview =
            resource_edit::apply(snapshot.text, std::slice::from_ref(&change.replacement))?;
        let root = resource_edit::parse(&preview, self.parser, self.cancellation, &mut budget)?;
        if !resource_edit::preserves(
            snapshot.root,
            &root,
            Some(&change),
            &BTreeMap::new(),
            self.cancellation,
        )? {
            return Err(failed(
                "heading rename would change other Markdown content or structure",
            ));
        }
        let before = calc_heading_identifiers(snapshot.root, self.heading_id_prefix);
        let after = calc_heading_identifiers(&root, self.heading_id_prefix);
        let changed = before != after;
        let known: HashSet<_> = before.iter().map(String::as_str).collect();
        let activated = after
            .iter()
            .filter(|identifier| !known.contains(identifier.as_str()))
            .cloned()
            .collect();
        let mut identifiers = BTreeMap::new();
        for (old, new) in before.into_iter().zip(after) {
            if old != new && !old.is_empty() {
                if new.is_empty() {
                    return Err(failed("rename would remove an addressable heading anchor"));
                }
                identifiers.insert(old, new);
            }
        }
        let path = files::file_uri_path(uri);
        let mut plan = Plan::Heading(HeadingPlan {
            uri: uri.to_string(),
            path: None,
            change,
            identifiers,
            activated,
        });
        if !changed || path.is_none() {
            let scope =
                Workspace::default().index_scope(self.cancellation, files::MAX_WORKSPACE_ROOTS)?;
            let source = SourceBuffer {
                uri: uri.to_string(),
                path: None,
                ambiguous: false,
            };
            let edits = resource_edit::rewrite(
                &snapshot,
                plan.heading(uri),
                |url| plan.destination(&source, url, &scope),
                self.parser,
                self.cancellation,
                &mut budget,
            )?;
            budget.document(uri)?;
            return Ok(vec![DocumentEdits {
                uri: uri.to_string(),
                version: Some(snapshot.version),
                edits,
            }]);
        }
        let scope = self
            .workspace
            .index_scope(self.cancellation, files::MAX_WORKSPACE_ROOTS)?;
        let path = scope.resolve(&path.unwrap()).ok_or_else(|| {
            failed(
                "heading anchor changes require a local workspace root containing the source file",
            )
        })?;
        if let Plan::Heading(plan) = &mut plan {
            plan.path = Some(path);
        }
        // Release the preview AST before scanning other documents.
        drop(root);
        self.execute(scope, plan, budget)
    }

    pub fn files(&mut self, files: &[FileRename]) -> Result<Vec<DocumentEdits>, ResponseError> {
        self.cancellation.check()?;
        let scope = self
            .workspace
            .index_scope(self.cancellation, files::MAX_WORKSPACE_ROOTS)?;
        let mut moves = Vec::new();
        let mut old_paths = BTreeSet::new();
        let mut new_paths = BTreeSet::new();
        for file in files {
            self.cancellation.check()?;
            let old = operation_path(&scope, &file.old_uri)?;
            let new = operation_path(&scope, &file.new_uri)?;
            if old == new {
                continue;
            }
            if !old_paths.insert(old.clone()) || !new_paths.insert(new.clone()) {
                return Err(failed(
                    "file rename sources and destinations must be unique",
                ));
            }
            if !files::is_file(&old) {
                return Err(failed("file rename requires an existing regular file"));
            }
            moves.push(FileMove {
                old,
                new,
                new_uri: file.new_uri.clone(),
            });
        }
        for item in &moves {
            if item
                .new
                .try_exists()
                .map_err(|_| failed("cannot inspect a file rename destination"))?
                && !old_paths.contains(&item.new)
            {
                return Err(failed("file rename destination already exists"));
            }
        }
        if moves.is_empty() {
            return Ok(Vec::new());
        }
        self.execute(scope, Plan::Files(moves), Budget::default())
    }

    fn execute(
        &mut self,
        scope: FileScope,
        plan: Plan,
        mut budget: Budget,
    ) -> Result<Vec<DocumentEdits>, ResponseError> {
        let sources = scope.sources(
            self.documents.keys().map(String::as_str),
            self.cancellation,
            files::MAX_WORKSPACE_ENTRIES,
            files::MAX_WORKSPACE_FILES,
        )?;
        if sources.buffers.iter().any(|buffer| buffer.ambiguous) {
            return Err(failed(
                "close duplicate URI aliases before a workspace rename",
            ));
        }
        let initial_files: BTreeSet<_> = sources.files.iter().cloned().collect();
        let mut remaining = files::MAX_WORKSPACE_BYTES;
        let mut result = Vec::new();
        let mut snapshots = Vec::new();
        let mut operation = Operation {
            plan: &plan,
            scope: &scope,
            parser: self.parser,
            cancellation: self.cancellation,
            budget: &mut budget,
        };
        for source in &sources.buffers {
            self.cancellation.check()?;
            self.used_buffers.insert(source.uri.clone());
            if let Plan::Files(moves) = &plan {
                if moves.iter().any(|item| {
                    source.path.as_ref() == Some(&item.old)
                        && files::file_uri_path(&source.uri).as_ref() != Some(&item.old)
                }) {
                    return Err(failed(
                        "close symlink alias buffers before moving their target file",
                    ));
                }
            }
            let document = self
                .documents
                .get_mut(&source.uri)
                .expect("selected buffer is open");
            remaining = remaining
                .checked_sub(document.text()?.len())
                .ok_or_else(|| failed("workspace rename exceeds the source byte limit"))?;
            let version = Some(document.version());
            let edits = operation.edit(document, source)?;
            if !edits.is_empty() {
                operation.budget.document(&source.uri)?;
                result.push(DocumentEdits {
                    uri: source.uri.clone(),
                    version,
                    edits,
                });
            }
        }
        for path in sources.files {
            self.cancellation.check()?;
            if scope.resolve(&path).as_ref() != Some(&path) {
                return Err(modified());
            }
            let text = files::read_markdown_budgeted(&path, &mut remaining)?.ok_or_else(|| {
                failed("cannot read every Markdown file required for this rename")
            })?;
            let mut document = Document::new(0, text)?;
            let uri = files::path_uri(&path).expect("scoped paths form file URIs");
            let source = SourceBuffer {
                uri: uri.clone(),
                path: Some(path.clone()),
                ambiguous: false,
            };
            let edits = operation.edit(&mut document, &source)?;
            if !edits.is_empty() {
                operation.budget.document(&uri)?;
                result.push(DocumentEdits {
                    uri,
                    version: None,
                    edits,
                });
            }
            snapshots.push((path, document.shared_text()?));
        }
        // Refactors cannot silently miss a new referrer or overwrite contents
        // changed during analysis. Closed files are revalidated as a whole.
        let current_scope = self
            .workspace
            .index_scope(self.cancellation, files::MAX_WORKSPACE_ROOTS)?;
        if current_scope != scope {
            return Err(modified());
        }
        let current = scope.sources(
            self.documents.keys().map(String::as_str),
            self.cancellation,
            files::MAX_WORKSPACE_ENTRIES,
            files::MAX_WORKSPACE_FILES,
        )?;
        if current.files.into_iter().collect::<BTreeSet<_>>() != initial_files
            || current.buffers != sources.buffers
        {
            return Err(modified());
        }
        let mut remaining = files::MAX_WORKSPACE_BYTES;
        for (path, expected) in snapshots {
            self.cancellation.check()?;
            if scope.resolve(&path).as_ref() != Some(&path)
                || files::read_markdown_budgeted(&path, &mut remaining)?.as_deref()
                    != Some(expected.as_str())
            {
                return Err(modified());
            }
        }
        if let Plan::Files(moves) = &plan {
            for item in moves {
                if !files::is_file(&item.old)
                    || scope.resolve(&item.old).as_ref() != Some(&item.old)
                    || scope.resolve(&item.new).as_ref() != Some(&item.new)
                    || (item.new.try_exists().map_err(|_| modified())?
                        && !moves.iter().any(|other| other.old == item.new))
                {
                    return Err(modified());
                }
            }
        }
        self.cancellation.check()?;
        result.sort_by(|left, right| left.uri.cmp(&right.uri));
        Ok(result)
    }
}

struct Operation<'a> {
    plan: &'a Plan,
    scope: &'a FileScope,
    parser: &'a YozoraParser,
    cancellation: &'a Cancellation,
    budget: &'a mut Budget,
}

impl Operation<'_> {
    fn edit(
        &mut self,
        document: &mut Document,
        source: &SourceBuffer,
    ) -> Result<Vec<TextEdit>, ResponseError> {
        self.cancellation.check()?;
        self.budget.parse(document.text()?.len())?;
        let snapshot = document.snapshot(self.parser, Position::default())?;
        self.cancellation.check()?;
        resource_edit::rewrite(
            &snapshot,
            self.plan.heading(&source.uri),
            |url| self.plan.destination(source, url, self.scope),
            self.parser,
            self.cancellation,
            self.budget,
        )
    }
}

fn operation_path(scope: &FileScope, uri: &str) -> Result<PathBuf, ResponseError> {
    if uri.contains(['?', '#']) {
        return Err(ResponseError::invalid_params(
            "file rename URIs cannot contain queries or fragments",
        ));
    }
    let Some(files::Target::Local {
        file: Some(file), ..
    }) = files::resolve(uri, uri, scope)
    else {
        return Err(failed(
            "file rename paths must be local files inside workspace roots",
        ));
    };
    if files::file_uri_path(uri).as_ref() != Some(&file.path) {
        return Err(failed(
            "file operations require canonical paths; symlink aliases are not renamed",
        ));
    }
    Ok(file.path)
}

fn relative_uri(parent: &Path, target: &Path) -> Result<String, ResponseError> {
    let from: Vec<_> = parent.components().collect();
    let to: Vec<_> = target.components().collect();
    let common = from
        .iter()
        .zip(&to)
        .take_while(|(left, right)| left == right)
        .count();
    if common == 0 {
        return files::path_uri(target).ok_or_else(|| failed("renamed path cannot form a URI"));
    }
    let mut components = vec!["..".to_string(); from.len() - common];
    for component in &to[common..] {
        let Component::Normal(value) = component else {
            return Err(failed("cannot make a relative file URI"));
        };
        components.push(files::encode_component(
            value
                .to_str()
                .ok_or_else(|| failed("file path is not Unicode"))?,
        ));
    }
    let path = components.join("/");
    if path.is_empty() {
        return Ok("./".to_string());
    }
    Ok(
        if path
            .split('/')
            .next()
            .is_some_and(|component| component.contains(':'))
        {
            format!("./{path}")
        } else {
            path
        },
    )
}

fn modified() -> ResponseError {
    ResponseError::new(
        -32801,
        "workspace files changed during rename; retry against the current files",
    )
}

#[cfg(test)]
mod tests;
