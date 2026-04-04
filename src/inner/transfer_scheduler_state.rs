use std::collections::{HashMap, HashSet, VecDeque};

use uuid::Uuid;

use super::active_state::ActiveState;
use super::group_state::GroupState;
use super::UniqueId;

pub(crate) struct TransferSchedulerState {
    max_upload_concurrency: usize,
    max_download_concurrency: usize,

    groups: HashMap<UniqueId, GroupState>,
    /// 任务实例 [`Uuid`] → 去重键，供 pause/cancel 解析。
    uuid_to_dedupe: HashMap<Uuid, UniqueId>,
    queued: VecDeque<UniqueId>,
    queued_set: HashSet<UniqueId>,
    active: HashMap<UniqueId, ActiveState>,
    offsets: HashMap<UniqueId, u64>,
}

impl TransferSchedulerState {
    pub(crate) fn new(max_upload_concurrency: usize, max_download_concurrency: usize) -> Self {
        Self {
            max_upload_concurrency,
            max_download_concurrency,
            groups: HashMap::new(),
            uuid_to_dedupe: HashMap::new(),
            queued: VecDeque::new(),
            queued_set: HashSet::new(),
            active: HashMap::new(),
            offsets: HashMap::new(),
        }
    }

    pub(crate) fn max_upload_concurrency(&self) -> usize {
        self.max_upload_concurrency
    }

    pub(crate) fn max_download_concurrency(&self) -> usize {
        self.max_download_concurrency
    }

    pub(crate) fn groups(&self) -> &HashMap<UniqueId, GroupState> {
        &self.groups
    }

    pub(crate) fn groups_mut(&mut self) -> &mut HashMap<UniqueId, GroupState> {
        &mut self.groups
    }

    pub(crate) fn queued(&self) -> &VecDeque<UniqueId> {
        &self.queued
    }

    pub(crate) fn queued_mut(&mut self) -> &mut VecDeque<UniqueId> {
        &mut self.queued
    }

    pub(crate) fn queued_set(&self) -> &HashSet<UniqueId> {
        &self.queued_set
    }

    pub(crate) fn queued_set_mut(&mut self) -> &mut HashSet<UniqueId> {
        &mut self.queued_set
    }

    pub(crate) fn active(&self) -> &HashMap<UniqueId, ActiveState> {
        &self.active
    }

    pub(crate) fn active_mut(&mut self) -> &mut HashMap<UniqueId, ActiveState> {
        &mut self.active
    }

    pub(crate) fn offsets(&self) -> &HashMap<UniqueId, u64> {
        &self.offsets
    }

    pub(crate) fn offsets_mut(&mut self) -> &mut HashMap<UniqueId, u64> {
        &mut self.offsets
    }

    pub(crate) fn uuid_to_dedupe(&self) -> &HashMap<Uuid, UniqueId> {
        &self.uuid_to_dedupe
    }

    pub(crate) fn uuid_to_dedupe_mut(&mut self) -> &mut HashMap<Uuid, UniqueId> {
        &mut self.uuid_to_dedupe
    }
}
