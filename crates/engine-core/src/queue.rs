use crate::job::{JobProgress, JobState, UpscaleJobRequest};
use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchJobItem {
    pub id: String,
    pub request: UpscaleJobRequest,
    pub state: JobState,
    pub progress: Option<JobProgress>,
    pub error_message: Option<String>,
    pub created_at_unix: u64,
}

impl BatchJobItem {
    pub fn new(request: UpscaleJobRequest) -> Self {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        Self {
            id: request.id.clone(),
            request,
            state: JobState::Queued,
            progress: None,
            error_message: None,
            created_at_unix: now,
        }
    }
}


pub struct BatchQueueManager {
    jobs: Vec<BatchJobItem>,
    is_active: bool,
    active_job_id: Option<String>,
}

impl BatchQueueManager {
    pub fn new() -> Self {
        Self {
            jobs: Vec::new(),
            is_active: false,
            active_job_id: None,
        }
    }

    pub fn add_job(&mut self, request: UpscaleJobRequest) -> String {
        let id = request.id.clone();
        self.jobs.push(BatchJobItem::new(request));
        id
    }

    pub fn remove_job(&mut self, job_id: &str) -> bool {
        if self.active_job_id.as_deref() == Some(job_id) {
            // Cannot remove actively rendering job directly; cancel first
            return false;
        }
        let initial_len = self.jobs.len();
        self.jobs.retain(|j| j.id != job_id);
        self.jobs.len() < initial_len
    }

    pub fn clear_completed(&mut self) {
        self.jobs.retain(|j| {
            j.state != JobState::Completed && j.state != JobState::Cancelled
        });
    }

    pub fn list_jobs(&self) -> Vec<BatchJobItem> {
        self.jobs.clone()
    }

    pub fn is_active(&self) -> bool {
        self.is_active
    }

    pub fn set_active(&mut self, active: bool) {
        self.is_active = active;
        if !active {
            self.active_job_id = None;
        }
    }

    pub fn get_active_job_id(&self) -> Option<String> {
        self.active_job_id.clone()
    }

    pub fn pop_next_pending_job(&mut self) -> Option<UpscaleJobRequest> {
        if !self.is_active {
            return None;
        }

        for item in &mut self.jobs {
            if item.state == JobState::Queued {
                item.state = JobState::Processing;
                self.active_job_id = Some(item.id.clone());
                return Some(item.request.clone());
            }
        }

        self.active_job_id = None;
        None
    }

    pub fn update_job_progress(&mut self, job_id: &str, progress: JobProgress) {
        if let Some(item) = self.jobs.iter_mut().find(|j| j.id == job_id) {
            item.state = progress.state;
            item.progress = Some(progress);
        }
    }

    pub fn mark_completed(&mut self, job_id: &str) {
        if let Some(item) = self.jobs.iter_mut().find(|j| j.id == job_id) {
            item.state = JobState::Completed;
            if let Some(ref mut p) = item.progress {
                p.state = JobState::Completed;
                p.percentage = 100.0;
            }
        }
        if self.active_job_id.as_deref() == Some(job_id) {
            self.active_job_id = None;
        }
    }

    pub fn mark_failed(&mut self, job_id: &str, error: String) {
        if let Some(item) = self.jobs.iter_mut().find(|j| j.id == job_id) {
            item.state = JobState::Failed;
            item.error_message = Some(error.clone());
            if let Some(ref mut p) = item.progress {
                p.state = JobState::Failed;
                p.error_message = Some(error);
            }
        }
        if self.active_job_id.as_deref() == Some(job_id) {
            self.active_job_id = None;
        }
    }

    pub fn mark_cancelled(&mut self, job_id: &str) {
        if let Some(item) = self.jobs.iter_mut().find(|j| j.id == job_id) {
            item.state = JobState::Cancelled;
            if let Some(ref mut p) = item.progress {
                p.state = JobState::Cancelled;
            }
        }
        if self.active_job_id.as_deref() == Some(job_id) {
            self.active_job_id = None;
        }
    }
}

impl Default for BatchQueueManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use inference_runtime::ExecutionBackend;
    use video_io::OutputEncodingProfile;

    fn sample_request(id: &str) -> UpscaleJobRequest {
        UpscaleJobRequest {
            id: id.to_string(),
            input_path: format!("in_{}.mp4", id),
            output_path: format!("out_{}.mp4", id),
            model_id: "compact-span-x2".to_string(),
            backend: ExecutionBackend::Cpu { thread_count: 4 },
            tile_size: None,
            tile_padding: None,
            film_grain: 0.1,
            temporal_mode: None,
            interpolation_mode: None,
            deinterlace_mode: None,
            denoise_strength: None,
            encoding_profile: OutputEncodingProfile::default(),
        }
    }

    #[test]
    fn test_queue_lifecycle() {
        let mut queue = BatchQueueManager::new();
        assert_eq!(queue.list_jobs().len(), 0);

        queue.add_job(sample_request("job-1"));
        queue.add_job(sample_request("job-2"));
        assert_eq!(queue.list_jobs().len(), 2);

        // Before queue is activated, pop returns None
        assert!(queue.pop_next_pending_job().is_none());

        queue.set_active(true);
        let next = queue.pop_next_pending_job().expect("Should pop job-1");
        assert_eq!(next.id, "job-1");
        assert_eq!(queue.get_active_job_id().as_deref(), Some("job-1"));

        queue.mark_completed("job-1");
        assert_eq!(queue.get_active_job_id(), None);

        let next2 = queue.pop_next_pending_job().expect("Should pop job-2");
        assert_eq!(next2.id, "job-2");

        queue.mark_completed("job-2");
        assert!(queue.pop_next_pending_job().is_none());

        queue.clear_completed();
        assert_eq!(queue.list_jobs().len(), 0);
    }
}
