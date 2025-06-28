import React from 'react';

interface ControlsPanelProps {
  isDrawing: boolean;
  onDrawingToggle: (drawing: boolean) => void;
  selectedTool: 'waypoint' | 'area' | 'path';
  onToolSelect: (tool: 'waypoint' | 'area' | 'path') => void;
  connectionStatus: 'connected' | 'disconnected' | 'connecting';
  onReconnect: () => void;
}

const ControlsPanel: React.FC<ControlsPanelProps> = ({
  isDrawing,
  onDrawingToggle,
  selectedTool,
  onToolSelect,
  connectionStatus,
  onReconnect
}) => {
  const getStatusColor = () => {
    switch (connectionStatus) {
      case 'connected': return '#4CAF50';
      case 'connecting': return '#FF9800';
      case 'disconnected': return '#f44336';
      default: return '#9E9E9E';
    }
  };

  return (
    <div className="controls-panel">
      <h3>Drawing Tools</h3>
      
      <div className="tool-selection">
        <label>
          <input
            type="radio"
            name="tool"
            value="waypoint"
            checked={selectedTool === 'waypoint'}
            onChange={() => onToolSelect('waypoint')}
          />
          Waypoint
        </label>
        <label>
          <input
            type="radio"
            name="tool"
            value="area"
            checked={selectedTool === 'area'}
            onChange={() => onToolSelect('area')}
          />
          Survey Area
        </label>
        <label>
          <input
            type="radio"
            name="tool"
            value="path"
            checked={selectedTool === 'path'}
            onChange={() => onToolSelect('path')}
          />
          Flight Path
        </label>
      </div>

      <div className="drawing-controls">
        <button
          className={isDrawing ? 'danger-button' : 'primary-button'}
          onClick={() => onDrawingToggle(!isDrawing)}
        >
          {isDrawing ? 'Stop Drawing' : 'Start Drawing'}
        </button>
      </div>

      <div className="connection-status">
        <h4>Connection Status</h4>
        <div style={{ 
          display: 'flex', 
          alignItems: 'center', 
          gap: '10px',
          marginBottom: '10px'
        }}>
          <div
            style={{
              width: '12px',
              height: '12px',
              borderRadius: '50%',
              backgroundColor: getStatusColor()
            }}
          />
          <span style={{ textTransform: 'capitalize' }}>
            {connectionStatus}
          </span>
        </div>
        
        {connectionStatus === 'disconnected' && (
          <button className="secondary-button" onClick={onReconnect}>
            Reconnect
          </button>
        )}
      </div>

      <div className="instructions">
        <h4>Instructions</h4>
        <ul style={{ fontSize: '12px', paddingLeft: '16px' }}>
          <li>Select a tool above</li>
          <li>Click "Start Drawing" to activate</li>
          <li>For waypoints: Click on map</li>
          <li>For areas: Click to draw polygon</li>
          <li>For paths: Click to draw line</li>
          <li>Save mission before deploying</li>
        </ul>
      </div>

      <div className="keyboard-shortcuts">
        <h4>Shortcuts</h4>
        <ul style={{ fontSize: '12px', paddingLeft: '16px' }}>
          <li><kbd>Ctrl+N</kbd> - New Mission</li>
          <li><kbd>Ctrl+S</kbd> - Save Mission</li>
          <li><kbd>Esc</kbd> - Stop Drawing</li>
          <li><kbd>Del</kbd> - Delete Selected</li>
        </ul>
      </div>
    </div>
  );
};

export default ControlsPanel;
