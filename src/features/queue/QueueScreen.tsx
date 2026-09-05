import { ListChecks } from "lucide-react";
import { useJobStore } from "../../stores/useJobStore";
import { JobRow } from "./JobRow";
import { EmptyState } from "../../components/common/EmptyState";
import "./QueueScreen.css";

const ACTIVE_STATUSES = new Set(["queued", "analyzing", "preparing", "converting", "finalizing"]);

export function QueueScreen() {
  const jobs = useJobStore((s) => s.jobs);
  const clearCompleted = useJobStore((s) => s.clearCompleted);

  const active = jobs.filter((j) => ACTIVE_STATUSES.has(j.status));
  const completed = jobs.filter((j) => j.status === "completed");
  const failed = jobs.filter((j) => j.status === "failed");
  const cancelled = jobs.filter((j) => j.status === "cancelled");

  if (jobs.length === 0) {
    return (
      <div className="screen-page">
        <EmptyState
          icon={ListChecks}
          title="No conversions in the queue"
          description="Files you convert from the Convert screen will show up here with live progress."
        />
      </div>
    );
  }

  return (
    <div className="screen-page">
      <header className="screen-page__header">
        <h1>Queue</h1>
        <p>Track active, completed, and failed conversions.</p>
      </header>

      <div className="queue-groups">
        {active.length > 0 && <QueueGroup title="Active" jobs={active} />}
        {failed.length > 0 && <QueueGroup title="Failed" jobs={failed} />}
        {completed.length > 0 && <QueueGroup title="Completed" jobs={completed} />}
        {cancelled.length > 0 && <QueueGroup title="Cancelled" jobs={cancelled} />}
      </div>

      {(completed.length > 0 || cancelled.length > 0) && (
        <div className="queue-footer">
          <button className="queue-footer__clear" onClick={clearCompleted}>
            Clear completed
          </button>
        </div>
      )}
    </div>
  );
}

function QueueGroup({ title, jobs }: { title: string; jobs: ReturnType<typeof useJobStore.getState>["jobs"] }) {
  return (
    <section className="queue-group">
      <h2 className="queue-group__title">
        {title} <span className="queue-group__count">{jobs.length}</span>
      </h2>
      <div className="queue-group__list">
        {jobs.map((job) => (
          <JobRow key={job.id} job={job} />
        ))}
      </div>
    </section>
  );
}
