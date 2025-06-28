import React, { useState, useEffect, useCallback } from 'react';
import MapComponent from './MapComponent';
import MissionPanel from './MissionPanel';
import ControlsPanel from './ControlsPanel';
import { Mission, Waypoint, WaypointType } from '../types/mission';
import { MissionService } from '../services/missionService';
import { WebSocketService } from '../services/websocketService';

const MissionPlanner: React.FC = () => {
  const [currentMission, setCurrentMission] = useState<Mission | null>(null);
  const [missions, setMissions] = useState<Mission[]>([]);
  const [isDrawing, setIsDrawing] = useState(false);
  const [selectedTool, setSelectedTool] = useState<'waypoint' | 'area' | 'path'>('waypoint');
  const [connectionStatus, setConnectionStatus] = useState<'connected' | 'disconnected' | 'connecting'>('disconnected');

  const missionService = new MissionService();
  const wsService = new WebSocketService();

  useEffect(() => {
    // Load missions on component mount
    loadMissions();
    
    // Setup WebSocket connection
    wsService.connect('ws://localhost:3001/ws');
    wsService.onConnectionChange(setConnectionStatus);
    
    return () => {
      wsService.disconnect();
    };
  }, []);

  const loadMissions = async () => {
    try {
      const missionList = await missionService.getMissions();
      setMissions(missionList);
    } catch (error) {
      console.error('Failed to load missions:', error);
    }
  };

  const createNewMission = useCallback(() => {
    const mission: Mission = {
      id: crypto.randomUUID(),
      name: `Mission ${new Date().toLocaleDateString()}`,
      description: 'New agricultural survey mission',
      created_at: new Date().toISOString(),
      updated_at: new Date().toISOString(),
      waypoints: [],
      area_of_interest: null,
      estimated_duration_minutes: 0,
      estimated_battery_usage: 0,
      weather_constraints: {
        max_wind_speed_ms: 15.0,
        max_precipitation_mm: 2.0,
        min_visibility_m: 1000.0,
        temperature_range_celsius: [-10.0, 45.0]
      },
      metadata: {}
    };
    setCurrentMission(mission);
  }, []);

  const saveMission = async () => {
    if (!currentMission) return;
    
    try {
      const savedMission = await missionService.saveMission(currentMission);
      setCurrentMission(savedMission);
      await loadMissions();
      alert('Mission saved successfully!');
    } catch (error) {
      console.error('Failed to save mission:', error);
      alert('Failed to save mission');
    }
  };

  const sendMissionToSystem = async () => {
    if (!currentMission) return;
    
    try {
      await wsService.sendMission(currentMission);
      alert('Mission sent to drone system!');
    } catch (error) {
      console.error('Failed to send mission:', error);
      alert('Failed to send mission');
    }
  };

  const addWaypoint = useCallback((lat: number, lng: number, altitude: number = 50) => {
    if (!currentMission) return;

    const waypoint: Waypoint = {
      id: crypto.randomUUID(),
      position: { lat, lng },
      altitude_m: altitude,
      waypoint_type: WaypointType.Navigation,
      actions: [],
      arrival_time: null,
      speed_ms: null,
      heading_degrees: null
    };

    setCurrentMission(prev => prev ? {
      ...prev,
      waypoints: [...prev.waypoints, waypoint],
      updated_at: new Date().toISOString()
    } : null);
  }, [currentMission]);

  const removeWaypoint = useCallback((waypointId: string) => {
    if (!currentMission) return;

    setCurrentMission(prev => prev ? {
      ...prev,
      waypoints: prev.waypoints.filter(wp => wp.id !== waypointId),
      updated_at: new Date().toISOString()
    } : null);
  }, [currentMission]);

  const optimizeMission = async () => {
    if (!currentMission) return;
    
    try {
      const optimized = await missionService.optimizeMission(currentMission.id);
      setCurrentMission(optimized);
      alert('Mission optimized!');
    } catch (error) {
      console.error('Failed to optimize mission:', error);
      alert('Failed to optimize mission');
    }
  };

  return (
    <div className="mission-planner">
      <div className="map-wrapper">
        <MapComponent
          mission={currentMission}
          onWaypointAdd={addWaypoint}
          onWaypointRemove={removeWaypoint}
          isDrawing={isDrawing}
          selectedTool={selectedTool}
        />
        
        <MissionPanel
          mission={currentMission}
          missions={missions}
          onMissionSelect={setCurrentMission}
          onNewMission={createNewMission}
          onSaveMission={saveMission}
          onSendMission={sendMissionToSystem}
          onOptimizeMission={optimizeMission}
        />
        
        <ControlsPanel
          isDrawing={isDrawing}
          onDrawingToggle={setIsDrawing}
          selectedTool={selectedTool}
          onToolSelect={setSelectedTool}
          connectionStatus={connectionStatus}
          onReconnect={() => wsService.connect('ws://localhost:3001/ws')}
        />
      </div>
    </div>
  );
};

export default MissionPlanner;
