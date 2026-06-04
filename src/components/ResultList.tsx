import React from 'react';
import { AppIcon } from './AppIcon';
import { InlineConfirm } from './InlineConfirm';
import { DeviceMultiSelect } from './DeviceMultiSelect';
import { SearchResult, DeviceOption } from '../types';

interface Props {
  results: SearchResult[];
  selectedIndex: number;
  onSelect: (result: SearchResult) => void;
  onHover: (index: number) => void;
  confirmingPath?: string | null;
  confirmMessage?: string;
  confirmLabel?: string;
  onConfirmTransfer?: () => void;
  onCancelTransfer?: () => void;
  expandedPath?: string | null;
  expandedActionIndex?: number;
  onActionHover?: (index: number) => void;
  onActionExecute?: (path: string, actionIndex: number) => void;
  sendToDevicesPath?: string | null;
  devices?: DeviceOption[];
  onCloseSendToDevices?: () => void;
  resultsAreFolders?: boolean;
}

const FILE_ACTIONS = ['Open', 'Reveal in file manager', 'Send to devices'];

function isRemote(source: SearchResult['source']): source is { Remote: { device_name: string } } {
  return typeof source === 'object' && 'Remote' in source;
}

function sourceLabel(source: SearchResult['source']): string {
  return isRemote(source) ? source.Remote.device_name : 'Local';
}

/** Generic 16x16 document glyph with a folded top-right corner, for file rows. */
function FileIcon(): React.ReactElement {
  return (
    <svg width={16} height={16} viewBox="0 0 16 16" fill="none" style={{ flexShrink: 0 }}>
      <path
        d="M4 2H9L12 5V14H4Z"
        stroke="var(--muted)"
        strokeWidth="1.2"
        strokeLinejoin="round"
      />
      <path d="M9 2V5H12" stroke="var(--muted)" strokeWidth="1.2" strokeLinejoin="round" />
    </svg>
  );
}

/** 16x16 folder glyph with a tab, for directory rows in folder search. */
function FolderIcon(): React.ReactElement {
  return (
    <svg width={16} height={16} viewBox="0 0 16 16" fill="none" style={{ flexShrink: 0 }}>
      <path
        d="M2 4.5C2 3.7 2.7 3 3.5 3H6L7.5 4.5H12.5C13.3 4.5 14 5.2 14 6V11.5C14 12.3 13.3 13 12.5 13H3.5C2.7 13 2 12.3 2 11.5V4.5Z"
        stroke="var(--muted)"
        strokeWidth="1.2"
        strokeLinejoin="round"
      />
    </svg>
  );
}

export const ResultList: React.FC<Props> = ({
  results,
  selectedIndex,
  onSelect,
  onHover,
  confirmingPath,
  confirmMessage,
  confirmLabel,
  onConfirmTransfer,
  onCancelTransfer,
  expandedPath,
  expandedActionIndex = 0,
  onActionHover,
  onActionExecute,
  sendToDevicesPath,
  devices = [],
  onCloseSendToDevices,
  resultsAreFolders = false,
}) => {
  return (
    <div>
      {results.map((result, index) => {
        const selected = index === selectedIndex;
        const isApp = result.result_type === 'App';
        const remote = isRemote(result.source);
        const confirming = confirmingPath != null && result.path === confirmingPath;
        const expanded = expandedPath != null && result.path === expandedPath;
        const sending = sendToDevicesPath != null && result.path === sendToDevicesPath;
        return (
          <React.Fragment key={`${result.path}-${index}`}>
            <div
              onClick={() => onSelect(result)}
              onMouseEnter={() => onHover(index)}
              className="flex items-center justify-between gap-2 px-3 py-2.5 cursor-pointer transition-colors"
              style={{
                position: 'relative',
                borderBottom: '1px solid var(--border)',
                backgroundColor: selected ? 'var(--surface-hover)' : 'transparent',
              }}
            >
              {confirming && (
                <InlineConfirm
                  message={confirmMessage ?? ''}
                  confirmLabel={confirmLabel}
                  onConfirm={() => onConfirmTransfer?.()}
                  onCancel={() => onCancelTransfer?.()}
                />
              )}
              <div className="flex items-center flex-1 min-w-0" style={{ gap: '10px' }}>
                {isApp ? (
                  <AppIcon iconPath={result.icon_path} size={20} />
                ) : (
                  <span
                    className="flex items-center justify-center shrink-0"
                    style={{ width: 20, height: 20 }}
                  >
                    {resultsAreFolders ? <FolderIcon /> : <FileIcon />}
                  </span>
                )}
                <div className="flex-1 min-w-0 flex flex-col gap-0.5">
                  <span
                    className="truncate text-[13px]"
                    style={{ color: 'var(--text)', fontWeight: isApp ? 500 : 400 }}
                  >
                    {result.name}
                  </span>
                  <span className="truncate text-xs" style={{ color: 'var(--muted)' }}>
                    {isApp ? 'Application' : result.path}
                  </span>
                </div>
              </div>
              {!isApp && (
                <span
                  className="text-[10px] px-1.5 py-0.5 rounded shrink-0"
                  style={
                    remote
                      ? {
                          backgroundColor: 'var(--surface-hover)',
                          color: 'var(--accent)',
                          border: '1px solid var(--accent)',
                        }
                      : {
                          backgroundColor: 'var(--surface-hover)',
                          color: 'var(--muted)',
                          border: '1px solid var(--border)',
                        }
                  }
                >
                  {sourceLabel(result.source)}
                </span>
              )}
            </div>

            {expanded && !sending && (
              <div
                style={{
                  backgroundColor: 'var(--surface)',
                  border: '1px solid color-mix(in srgb, var(--accent) 30%, transparent)',
                  borderTop: '1px solid var(--border)',
                  borderRadius: '0 0 8px 8px',
                  padding: '4px 0',
                }}
              >
                {FILE_ACTIONS.map((label, actionIndex) => {
                  const actionSelected = actionIndex === expandedActionIndex;
                  return (
                    <div
                      key={label}
                      onMouseEnter={() => onActionHover?.(actionIndex)}
                      onClick={e => {
                        e.stopPropagation();
                        onActionExecute?.(result.path, actionIndex);
                      }}
                      className="cursor-pointer transition-colors"
                      style={{
                        padding: '8px 16px 8px 36px',
                        fontSize: '13px',
                        color: actionSelected ? 'var(--accent)' : 'var(--text)',
                        backgroundColor: actionSelected
                          ? 'color-mix(in srgb, var(--accent) 10%, transparent)'
                          : 'transparent',
                      }}
                    >
                      {label}
                    </div>
                  );
                })}
              </div>
            )}

            {sending && (
              <DeviceMultiSelect
                devices={devices}
                filePath={result.path}
                onClose={() => onCloseSendToDevices?.()}
              />
            )}
          </React.Fragment>
        );
      })}
    </div>
  );
};
