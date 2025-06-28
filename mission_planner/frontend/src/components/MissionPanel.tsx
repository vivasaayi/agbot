import React from 'react';
import { Mission } from '../types/mission';

interface MissionPanelProps {
  mission: Mission | null;
  missions: Mission[];
  onMissionSelect: (mission: Mission) => void;
  onNewMission: () => void;
  onSaveMission: () => void;
  onSendMission: () => void;
  onOptimizeMission: () => void;
}

const MissionPanel: React.FC<MissionPanelProps> = ({
  mission,
  missions,
  onMissionSelect,
  onNewMission,
  onSaveMission,
  onSendMission,
  onOptimizeMission
}) => {
  return (
    <div className="mission-panel">
      <h3>Mission Planning</h3>
      
      <div className="button-group">
        <button className="primary-button" onClick={onNewMission}>
          New Mission
        </button>
        {mission && (
          <>
            <button className="secondary-button" onClick={onSaveMission}>
              Save Mission
            </button>
            <button className="secondary-button" onClick={onOptimizeMission}>
              Optimize
            </button>
            <button className="primary-button" onClick={onSendMission}>
              Deploy Mission
            </button>
          </>
        )}
      </div>

      {missions.length > 0 && (
        <div className="mission-list">
          <h4>Saved Missions</h4>
          <select 
            value={mission?.id || ''} 
            onChange={(e) => {
              const selectedMission = missions.find(m => m.id === e.target.value);
              if (selectedMission) {
                onMissionSelect(selectedMission);
              }
            }}
          >
            <option value="">Select a mission...</option>
            {missions.map(m => (
              <option key={m.id} value={m.id}>
                {m.name} ({m.waypoints.length} waypoints)
              </option>
            ))}
          </select>
        </div>
      )}

      {mission && (
        <div className="mission-details">
          <h4>Current Mission</h4>
          <div className="mission-info">
            <p><strong>Name:</strong> {mission.name}</p>
            <p><strong>Waypoints:</strong> {mission.waypoints.length}</p>
            <p><strong>Est. Duration:</strong> {mission.estimated_duration_minutes} min</p>
            <p><strong>Est. Battery:</strong> {(mission.estimated_battery_usage * 100).toFixed(1)}%</p>
          </div>
          
          <div className="waypoint-list">
            <h5>Waypoints</h5>
            {mission.waypoints.length === 0 ? (
              <p>No waypoints yet. Click on the map to add waypoints.</p>
            ) : (
              mission.waypoints.map((waypoint, index) => (
                <div key={waypoint.id} className="waypoint-item">
                  <strong>#{index + 1}</strong> {waypoint.waypoint_type}<br />
                  <small>
                    {waypoint.position.lat.toFixed(6)}, {waypoint.position.lng.toFixed(6)} @ {waypoint.altitude_m}m
                  </small>
                  {waypoint.actions.length > 0 && (
                    <div>
                      <small>Actions: {waypoint.actions.map(a => a.type).join(', ')}</small>
                    </div>
                  )}
                </div>
              ))
            )}
          </div>

          <div className="weather-constraints">
            <h5>Weather Constraints</h5>
            <p><strong>Max Wind:</strong> {mission.weather_constraints.max_wind_speed_ms} m/s</p>
            <p><strong>Max Rain:</strong> {mission.weather_constraints.max_precipitation_mm} mm</p>
            <p><strong>Min Visibility:</strong> {mission.weather_constraints.min_visibility_m} m</p>
            <p><strong>Temperature:</strong> {mission.weather_constraints.temperature_range_celsius[0]}°C to {mission.weather_constraints.temperature_range_celsius[1]}°C</p>
          </div>
        </div>
      )}
    </div>
  );
};

export default MissionPanel;
