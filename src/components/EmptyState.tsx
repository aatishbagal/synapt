import React from 'react';

interface Props {
  title: string;
  /** One or more lines of secondary text. */
  detail?: React.ReactNode;
  /** Optional action, rendered 12px below the text. */
  action?: React.ReactNode;
}

/** The DESIGN.md empty state: centred title, muted detail, optional action. */
export const EmptyState: React.FC<Props> = ({ title, detail, action }) => (
  <div className="empty-state">
    <p className="empty-state-title">{title}</p>
    {detail && <div className="empty-state-detail">{detail}</div>}
    {action && <div className="mt-3">{action}</div>}
  </div>
);
