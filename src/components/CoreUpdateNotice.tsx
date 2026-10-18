import type { CoreUpdateStatus } from '../hooks/useCoreUpdate';

interface CoreUpdateNoticeProps {
  status: CoreUpdateStatus | null;
  checking: boolean;
  error: string | null;
}

export function CoreUpdateNotice({ status, checking, error }: CoreUpdateNoticeProps) {
  const warning = status?.compatibility_warning;
  const checkError = error ?? status?.check_error;
  return (
    <div className="flex flex-col gap-1 pt-2" style={{ borderTop: '1px solid var(--border)' }} aria-live="polite">
      <p className="text-xs" style={{ color: 'var(--text)' }}>
        SynaptCore{status ? ` ${status.bundled_version}` : ''}
      </p>
      {warning && <p role="alert" className="text-xs" style={{ color: 'var(--danger)' }}>{warning}</p>}
      {status?.update_available && (
        <p role="status" className="text-xs" style={{ color: 'var(--text)' }}>
          SynaptCore {status.latest_version} is available. This app includes an older core.
          {status.latest_app_compatible === false ? ' The new core requires a compatible app release.' : ''}
        </p>
      )}
      <p className="text-xs" style={{ color: checkError ? 'var(--danger)' : 'var(--muted)' }}>
        {checking ? 'Checking SynaptCore...' : checkError ? `Could not check SynaptCore: ${checkError}`
          : status?.latest_version ? (status.update_available ? 'Core updates are included in app updates.'
            : status.compatible ? `Core is compatible. Latest release: ${status.latest_version}.` : 'App and core compatibility needs attention.')
          : status ? 'Core updates not checked yet.' : 'Loading core version...'}
      </p>
    </div>
  );
}
