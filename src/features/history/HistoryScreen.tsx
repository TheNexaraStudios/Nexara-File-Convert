import { History } from "lucide-react";
import { useJobStore } from "../../stores/useJobStore";
import { JobRow } from "../queue/JobRow";
import { EmptyState } from "../../components/common/EmptyState";
import "../../styles/screen.css";
import "../queue/QueueScreen.css";

const TERMINAL_STATUSES = new Set(["completed", "failed", "cancelled"]);

export function HistoryScreen() {
  const jobs = useJobStore((s) => s.jobs);
  const clearHistory = useJobStore((s) => s.clearHistory);

  const history = jobs
    .filter((j) => TERMINAL_STATUSES.has(j.status))
    .sort((a, b) => (b.completedAt ?? b.createdAt) - (a.completedAt ?? a.createdAt));

  if (history.length === 0) {
    return (
      <div className="screen-page">
        <header className="screen-page__header">
          <h1>History</h1>
          <p>A local record of everything you've converted.</p>
        </header>
        <EmptyState
          icon={History}
          title="No conversions yet"
          description="Your conversion history will appear here — stored locally, never uploaded anywhere."
        />
      </div>
    );
  }

  return (
    <div className="screen-page">
      <header className="screen-page__header">
        <h1>History</h1>
        <p>A local record of everything you've converted.</p>
      </header>

      <div className="queue-groups">
        <section className="queue-group">
          <div className="queue-group__list">
            {history.map((job) => (
              <JobRow key={job.id} job={job} />
            ))}
          </div>
        </section>
      </div>

      <div className="queue-footer">
        <button className="queue-footer__clear" onClick={clearHistory}>
          Clear history
        </button>
      </div>
    </div>
  );
}
