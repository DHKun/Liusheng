//! Qt list models consume immutable snapshots and emit bounded structural/data changes.
use cxx_qt::CxxQtType;
use cxx_qt_lib::{
    QByteArray, QHash, QHashPair_i32_QByteArray, QModelIndex, QString, QVariant, QVector,
};
use liusheng_core::library::{AlbumSummary, ArtistSummary, TrackRow};
use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};

#[derive(Clone, PartialEq)]
pub enum RowValue {
    Track(Arc<TrackRow>),
    Album {
        summary: Arc<AlbumSummary>,
        cover: String,
        search: String,
    },
    Artist {
        summary: Arc<ArtistSummary>,
        cover: String,
        search: String,
    },
}
#[derive(Clone, PartialEq)]
pub struct ModelRow {
    pub key: String,
    pub source: i32,
    pub value: RowValue,
}
impl ModelRow {
    fn search(&self) -> &str {
        match &self.value {
            RowValue::Track(t) => &t.search_text,
            RowValue::Album { search, .. } | RowValue::Artist { search, .. } => search,
        }
    }
    fn text(&self, role: i32) -> String {
        match (&self.value, role) {
            (_, 256) => self.key.clone(),
            (RowValue::Track(t), 258) => t.title.clone(),
            (RowValue::Track(t), 259) => {
                if t.artist.is_empty() {
                    "未知艺术家".into()
                } else {
                    t.artist.clone()
                }
            }
            (RowValue::Track(t), 260) => {
                if t.album.is_empty() {
                    "未知专辑".into()
                } else {
                    t.album.clone()
                }
            }
            (RowValue::Track(t), 261) => match (t.disc_no, t.track_no) {
                (Some(d), Some(n)) if d > 1 => format!("{d}-{n:02}"),
                (_, Some(n)) => format!("{n:02}"),
                _ => format!("{:02}", self.source + 1),
            },
            (RowValue::Track(t), 263) => t.path.clone(),
            (RowValue::Album { summary, .. }, 258) => summary.title.clone(),
            (RowValue::Album { summary, .. }, 259) => summary.artist.clone(),
            (RowValue::Album { cover, .. }, 264) | (RowValue::Artist { cover, .. }, 264) => {
                cover.clone()
            }
            (RowValue::Artist { summary, .. }, 258) => summary.name.clone(),
            _ => String::new(),
        }
    }
    fn number(&self, role: i32) -> i32 {
        match (&self.value, role) {
            (_, 257) => self.source,
            (RowValue::Track(t), 262) => t.duration_ms.min(i32::MAX as u64) as i32,
            (RowValue::Track(t), 267) => t.year.unwrap_or(0) as i32,
            (RowValue::Album { summary, .. }, 265) => summary.track_count as i32,
            (RowValue::Album { summary, .. }, 267) => summary.year.unwrap_or(0) as i32,
            (RowValue::Artist { summary, .. }, 265) => summary.track_count as i32,
            (RowValue::Artist { summary, .. }, 266) => summary.album_count as i32,
            _ => 0,
        }
    }
}
// A desktop process owns one controller; the registry contains data, never Qt objects.
type Store = HashMap<String, Arc<Vec<ModelRow>>>;
fn store() -> &'static Mutex<Store> {
    static STORE: OnceLock<Mutex<Store>> = OnceLock::new();
    STORE.get_or_init(|| Mutex::new(HashMap::new()))
}
pub fn publish(kind: &str, rows: Vec<ModelRow>) {
    store()
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .insert(kind.to_owned(), Arc::new(rows));
}
pub fn update_covers(kind: &str, covers: &[String]) {
    let mut store = store().lock().unwrap_or_else(|e| e.into_inner());
    let Some(rows) = store.get_mut(kind) else {
        return;
    };
    if rows.iter().all(|row| match &row.value {
        RowValue::Album { cover, .. } | RowValue::Artist { cover, .. } => {
            covers.get(row.source as usize) == Some(cover)
        }
        _ => true,
    }) {
        return;
    }
    for row in Arc::make_mut(rows) {
        match &mut row.value {
            RowValue::Album { cover, .. } | RowValue::Artist { cover, .. } => {
                if let Some(url) = covers.get(row.source as usize) {
                    cover.clone_from(url);
                }
            }
            _ => {}
        }
    }
}
#[derive(Default)]
pub struct UiModelRust {
    kind: QString,
    query: QString,
    rows: Vec<ModelRow>,
    snapshot: Option<Arc<Vec<ModelRow>>>,
    applied_query: String,
}

#[cxx_qt::bridge]
pub mod qobject {
    unsafe extern "C++Qt" {
        include!(<QtCore/QAbstractListModel>);
        #[qobject]
        type QAbstractListModel;
    }
    unsafe extern "C++" {
        include!("cxx-qt-lib/qstring.h");
        type QString = cxx_qt_lib::QString;
        include!("cxx-qt-lib/qvariant.h");
        type QVariant = cxx_qt_lib::QVariant;
        include!("cxx-qt-lib/qmodelindex.h");
        type QModelIndex = cxx_qt_lib::QModelIndex;
        include!("cxx-qt-lib/qvector.h");
        type QVector_i32 = cxx_qt_lib::QVector<i32>;
        include!("cxx-qt-lib/qhash.h");
        type QHash_i32_QByteArray = cxx_qt_lib::QHash<cxx_qt_lib::QHashPair_i32_QByteArray>;
    }
    extern "RustQt" {
        #[qobject]
        #[qml_element]
        #[base = QAbstractListModel]
        #[qproperty(QString, kind)]
        #[qproperty(QString, query)]
        #[namespace = "liusheng"]
        type UiModel = super::UiModelRust;
        #[qinvokable]
        fn refresh(self: Pin<&mut UiModel>);
        #[qinvokable]
        #[cxx_override]
        fn data(self: &UiModel, index: &QModelIndex, role: i32) -> QVariant;
        #[qinvokable]
        #[cxx_override]
        #[cxx_name = "rowCount"]
        fn row_count(self: &UiModel, parent: &QModelIndex) -> i32;
        #[qinvokable]
        #[cxx_override]
        #[cxx_name = "roleNames"]
        fn role_names(self: &UiModel) -> QHash_i32_QByteArray;
        /// # Safety
        /// Every successful begin operation is paired immediately with its matching end operation.
        #[inherit]
        #[cxx_name = "beginInsertRows"]
        unsafe fn begin_insert_rows(
            self: Pin<&mut UiModel>,
            parent: &QModelIndex,
            first: i32,
            last: i32,
        );
        /// # Safety
        /// Called only after beginInsertRows and the announced insertion.
        #[inherit]
        #[cxx_name = "endInsertRows"]
        unsafe fn end_insert_rows(self: Pin<&mut UiModel>);
        /// # Safety
        /// Every successful begin operation is paired immediately with its matching end operation.
        #[inherit]
        #[cxx_name = "beginRemoveRows"]
        unsafe fn begin_remove_rows(
            self: Pin<&mut UiModel>,
            parent: &QModelIndex,
            first: i32,
            last: i32,
        );
        /// # Safety
        /// Called only after beginRemoveRows and the announced removal.
        #[inherit]
        #[cxx_name = "endRemoveRows"]
        unsafe fn end_remove_rows(self: Pin<&mut UiModel>);
    }
    unsafe extern "RustQt" {
        #[inherit]
        fn index(self: &UiModel, row: i32, column: i32, parent: &QModelIndex) -> QModelIndex;
        #[inherit]
        #[qsignal]
        #[cxx_name = "dataChanged"]
        fn data_changed(
            self: Pin<&mut UiModel>,
            top_left: &QModelIndex,
            bottom_right: &QModelIndex,
            roles: &QVector_i32,
        );
    }
}
impl qobject::UiModel {
    pub fn row_count(&self, parent: &QModelIndex) -> i32 {
        if parent.is_valid() {
            0
        } else {
            self.rust().rows.len().min(i32::MAX as usize) as i32
        }
    }
    pub fn role_names(&self) -> QHash<QHashPair_i32_QByteArray> {
        let mut roles = QHash::default();
        for (i, name) in [
            "stableId",
            "sourceIndex",
            "rowTitle",
            "rowArtist",
            "rowAlbum",
            "rowNumber",
            "rowDuration",
            "rowPath",
            "rowCover",
            "rowTrackCount",
            "rowAlbumCount",
            "rowYear",
        ]
        .iter()
        .enumerate()
        {
            roles.insert(256 + i as i32, QByteArray::from(*name));
        }
        roles
    }
    pub fn data(&self, index: &QModelIndex, role: i32) -> QVariant {
        if !index.is_valid() || index.column() != 0 {
            return QVariant::default();
        }
        let Some(row) = usize::try_from(index.row())
            .ok()
            .and_then(|i| self.rust().rows.get(i))
        else {
            return QVariant::default();
        };
        if matches!(role, 257 | 262 | 265 | 266 | 267) {
            QVariant::from(&row.number(role))
        } else {
            QVariant::from(&QString::from(&row.text(role)))
        }
    }
    pub fn refresh(mut self: core::pin::Pin<&mut Self>) {
        let snapshot = store()
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .get(&self.kind().to_string())
            .cloned()
            .unwrap_or_default();
        let query = liusheng_core::library::pinyin::normalize(&self.query().to_string());
        if self
            .rust()
            .snapshot
            .as_ref()
            .is_some_and(|s| Arc::ptr_eq(s, &snapshot))
            && self.rust().applied_query == query
        {
            return;
        }
        let rows = snapshot
            .iter()
            .filter(|r| query.is_empty() || r.search().contains(&query))
            .cloned()
            .collect::<Vec<_>>();
        let (prefix, suffix) = common_edges(&self.rust().rows, &rows);
        let old_len = self.rust().rows.len();
        let removed = old_len - prefix - suffix;
        let added = rows.len() - prefix - suffix;
        // Allocate insertion rows and replacement storage before entering Qt begin/end pairs.
        let insertion = rows[prefix..prefix + added].to_vec();
        self.as_mut().rust_mut().get_mut().rows.reserve(added);
        if removed > 0 {
            // SAFETY: announced valid row range is removed between this matched begin/end pair.
            unsafe {
                self.as_mut().begin_remove_rows(
                    &QModelIndex::default(),
                    prefix as i32,
                    (prefix + removed - 1) as i32,
                );
            }
            self.as_mut()
                .rust_mut()
                .get_mut()
                .rows
                .drain(prefix..prefix + removed);
            unsafe {
                self.as_mut().end_remove_rows();
            }
        }
        if added > 0 {
            // SAFETY: storage is pre-reserved and exactly the announced rows are inserted.
            unsafe {
                self.as_mut().begin_insert_rows(
                    &QModelIndex::default(),
                    prefix as i32,
                    (prefix + added - 1) as i32,
                );
            }
            self.as_mut()
                .rust_mut()
                .get_mut()
                .rows
                .splice(prefix..prefix, insertion);
            unsafe {
                self.as_mut().end_insert_rows();
            }
        }
        let changed = self
            .rust()
            .rows
            .iter()
            .zip(&rows)
            .enumerate()
            .filter_map(|(i, (a, b))| (a != b).then_some(i))
            .collect::<Vec<_>>();
        {
            let state = self.as_mut().rust_mut();
            let state = state.get_mut();
            state.rows = rows;
            state.snapshot = Some(snapshot);
            state.applied_query = query;
        }
        // Single metadata edits notify a single row; contiguous edits are coalesced.
        let mut groups = Vec::<(usize, usize)>::new();
        for i in changed {
            if let Some((_, end)) = groups.last_mut().filter(|(_, e)| *e + 1 == i) {
                *end = i;
            } else {
                groups.push((i, i));
            }
        }
        for (a, b) in groups {
            let top = self.index(a as i32, 0, &QModelIndex::default());
            let bottom = self.index(b as i32, 0, &QModelIndex::default());
            self.as_mut()
                .data_changed(&top, &bottom, &QVector::default());
        }
    }
}
fn common_edges(old: &[ModelRow], new: &[ModelRow]) -> (usize, usize) {
    let prefix = old
        .iter()
        .zip(new)
        .take_while(|(a, b)| a.key == b.key)
        .count();
    let suffix = old[prefix..]
        .iter()
        .rev()
        .zip(new[prefix..].iter().rev())
        .take_while(|(a, b)| a.key == b.key)
        .count();
    (prefix, suffix)
}
#[cfg(test)]
mod tests {
    use super::*;
    fn rows(ids: &[&str]) -> Vec<ModelRow> {
        ids.iter()
            .enumerate()
            .map(|(i, id)| ModelRow {
                key: id.to_string(),
                source: i as i32,
                value: RowValue::Artist {
                    summary: Arc::new(ArtistSummary {
                        key: id.to_string(),
                        name: id.to_string(),
                        track_count: 0,
                        album_count: 0,
                    }),
                    cover: String::new(),
                    search: id.to_string(),
                },
            })
            .collect()
    }
    #[test]
    fn stable_edges_isolate_insertions_and_deletions() {
        assert_eq!(
            common_edges(&rows(&["a", "c"]), &rows(&["a", "b", "c"])),
            (1, 1)
        );
        assert_eq!(
            common_edges(&rows(&["a", "b", "c"]), &rows(&["a", "c"])),
            (1, 1)
        );
    }
    #[test]
    fn same_ids_keep_structure_for_metadata_changes() {
        assert_eq!(common_edges(&rows(&["a", "b"]), &rows(&["a", "b"])), (2, 0));
    }
}
