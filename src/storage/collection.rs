use super::request::ApiRequest;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::Path;
use uuid::Uuid;

/// An item in a collection (either a request or a folder)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum CollectionItem {
    Request(ApiRequest),
    Folder {
        id: String,
        name: String,
        items: Vec<CollectionItem>,
        expanded: bool,
    },
}

impl CollectionItem {
    pub fn new_folder(name: impl Into<String>) -> Self {
        CollectionItem::Folder {
            id: Uuid::new_v4().to_string(),
            name: name.into(),
            items: Vec::new(),
            expanded: true,
        }
    }

    pub fn id(&self) -> &str {
        match self {
            CollectionItem::Request(req) => &req.id,
            CollectionItem::Folder { id, .. } => id,
        }
    }

    pub fn name(&self) -> &str {
        match self {
            CollectionItem::Request(req) => &req.name,
            CollectionItem::Folder { name, .. } => name,
        }
    }

    pub fn is_folder(&self) -> bool {
        matches!(self, CollectionItem::Folder { .. })
    }

    pub fn set_name(&mut self, new_name: impl Into<String>) {
        match self {
            CollectionItem::Request(req) => req.name = new_name.into(),
            CollectionItem::Folder { name, .. } => *name = new_name.into(),
        }
    }
}

/// A collection of API requests
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Collection {
    pub id: String,
    pub name: String,
    pub items: Vec<CollectionItem>,
    #[serde(skip)]
    pub expanded: bool,
}

impl Collection {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            name: name.into(),
            items: Vec::new(),
            expanded: true,
        }
    }

    /// Load a collection from a JSON file
    pub fn load(path: &Path) -> Result<Self> {
        let content = std::fs::read_to_string(path)?;
        let mut collection: Collection = serde_json::from_str(&content)?;
        collection.expanded = true;
        Ok(collection)
    }

    /// Save the collection to a JSON file
    pub fn save(&self, path: &Path) -> Result<()> {
        let content = serde_json::to_string_pretty(self)?;
        std::fs::write(path, content)?;
        Ok(())
    }

    /// Add a request to the collection
    pub fn add_request(&mut self, request: ApiRequest) {
        self.items.push(CollectionItem::Request(request));
    }

    /// Add a folder to the collection
    pub fn add_folder(&mut self, name: impl Into<String>) {
        self.items.push(CollectionItem::new_folder(name));
    }

    /// Get a flat list of all requests in the collection (for display)
    pub fn flatten(&self) -> Vec<(usize, &CollectionItem)> {
        let mut result = Vec::new();
        Self::flatten_items(&self.items, 0, &mut result);
        result
    }

    fn flatten_items<'a>(
        items: &'a [CollectionItem],
        depth: usize,
        result: &mut Vec<(usize, &'a CollectionItem)>,
    ) {
        for item in items {
            result.push((depth, item));
            if let CollectionItem::Folder { items, expanded, .. } = item {
                if *expanded {
                    Self::flatten_items(items, depth + 1, result);
                }
            }
        }
    }

    /// Find a request by ID
    pub fn find_request(&self, id: &str) -> Option<&ApiRequest> {
        Self::find_request_in_items(&self.items, id)
    }

    fn find_request_in_items<'a>(items: &'a [CollectionItem], id: &str) -> Option<&'a ApiRequest> {
        for item in items {
            match item {
                CollectionItem::Request(req) if req.id == id => return Some(req),
                CollectionItem::Folder { items, .. } => {
                    if let Some(req) = Self::find_request_in_items(items, id) {
                        return Some(req);
                    }
                }
                _ => {}
            }
        }
        None
    }

    /// Find and update a request by ID
    pub fn update_request(&mut self, id: &str, mut f: impl FnMut(&mut ApiRequest)) -> bool {
        Self::update_request_in_items(&mut self.items, id, &mut f)
    }

    fn update_request_in_items(
        items: &mut [CollectionItem],
        id: &str,
        f: &mut impl FnMut(&mut ApiRequest),
    ) -> bool {
        for item in items {
            match item {
                CollectionItem::Request(req) if req.id == id => {
                    f(req);
                    return true;
                }
                CollectionItem::Folder { items, .. } => {
                    if Self::update_request_in_items(items, id, f) {
                        return true;
                    }
                }
                _ => {}
            }
        }
        false
    }

    /// Rename the collection
    pub fn rename(&mut self, new_name: impl Into<String>) {
        self.name = new_name.into();
    }

    /// Add a request to a specific folder (or root if folder_id is None)
    pub fn add_request_to(&mut self, request: ApiRequest, folder_id: Option<&str>) -> bool {
        match folder_id {
            None => {
                self.items.push(CollectionItem::Request(request));
                true
            }
            Some(id) => Self::add_request_to_folder(&mut self.items, request, id),
        }
    }

    fn add_request_to_folder(items: &mut [CollectionItem], request: ApiRequest, folder_id: &str) -> bool {
        for item in items {
            if let CollectionItem::Folder { id, items: folder_items, .. } = item {
                if id == folder_id {
                    folder_items.push(CollectionItem::Request(request));
                    return true;
                }
                if Self::add_request_to_folder(folder_items, request.clone(), folder_id) {
                    return true;
                }
            }
        }
        false
    }

    /// Add a folder to a specific parent folder (or root if parent_id is None)
    pub fn add_folder_to(&mut self, name: impl Into<String>, parent_id: Option<&str>) -> bool {
        let new_folder = CollectionItem::new_folder(name);
        match parent_id {
            None => {
                self.items.push(new_folder);
                true
            }
            Some(id) => Self::add_folder_to_parent(&mut self.items, new_folder, id),
        }
    }

    fn add_folder_to_parent(items: &mut [CollectionItem], new_folder: CollectionItem, parent_id: &str) -> bool {
        for item in items {
            if let CollectionItem::Folder { id, items: folder_items, .. } = item {
                if id == parent_id {
                    folder_items.push(new_folder);
                    return true;
                }
                if Self::add_folder_to_parent(folder_items, new_folder.clone(), parent_id) {
                    return true;
                }
            }
        }
        false
    }

    /// Delete an item by ID (request or folder)
    pub fn delete_item(&mut self, item_id: &str) -> bool {
        Self::delete_item_recursive(&mut self.items, item_id)
    }

    fn delete_item_recursive(items: &mut Vec<CollectionItem>, item_id: &str) -> bool {
        // First check if item is at this level
        if let Some(pos) = items.iter().position(|item| item.id() == item_id) {
            items.remove(pos);
            return true;
        }
        // Otherwise search in subfolders
        for item in items {
            if let CollectionItem::Folder { items: folder_items, .. } = item {
                if Self::delete_item_recursive(folder_items, item_id) {
                    return true;
                }
            }
        }
        false
    }

    /// Rename an item by ID
    pub fn rename_item(&mut self, item_id: &str, new_name: impl Into<String>) -> bool {
        Self::rename_item_recursive(&mut self.items, item_id, new_name.into())
    }

    fn rename_item_recursive(items: &mut [CollectionItem], item_id: &str, new_name: String) -> bool {
        for item in items {
            match item {
                CollectionItem::Request(req) if req.id == item_id => {
                    req.name = new_name;
                    return true;
                }
                CollectionItem::Folder { id, name, items: folder_items, .. } if id == item_id => {
                    *name = new_name;
                    return true;
                }
                CollectionItem::Folder { items: folder_items, .. } => {
                    if Self::rename_item_recursive(folder_items, item_id, new_name.clone()) {
                        return true;
                    }
                }
                _ => {}
            }
        }
        false
    }
}

impl Default for Collection {
    fn default() -> Self {
        Self::new("New Collection")
    }
}
