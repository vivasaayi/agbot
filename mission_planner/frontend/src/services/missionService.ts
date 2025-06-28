import axios from 'axios';
import { Mission, MAVLinkMission, MAVLinkCommand, MAV_CMD, WaypointType } from '../types/mission';

export class MissionService {
  private baseUrl = process.env.REACT_APP_API_URL || 'http://localhost:3000/api';

  async getMissions(): Promise<Mission[]> {
    const response = await axios.get(`${this.baseUrl}/missions`);
    return response.data;
  }

  async getMission(id: string): Promise<Mission> {
    const response = await axios.get(`${this.baseUrl}/missions/${id}`);
    return response.data;
  }

  async saveMission(mission: Mission): Promise<Mission> {
    if (mission.id) {
      const response = await axios.put(`${this.baseUrl}/missions/${mission.id}`, mission);
      return response.data;
    } else {
      const response = await axios.post(`${this.baseUrl}/missions`, mission);
      return response.data;
    }
  }

  async deleteMission(id: string): Promise<void> {
    await axios.delete(`${this.baseUrl}/missions/${id}`);
  }

  async optimizeMission(id: string): Promise<Mission> {
    const response = await axios.post(`${this.baseUrl}/missions/${id}/optimize`);
    return response.data;
  }

  async convertToMAVLink(mission: Mission): Promise<MAVLinkMission> {
    const items: MAVLinkCommand[] = [];

    // Add takeoff command if we have waypoints
    if (mission.waypoints.length > 0) {
      const firstWaypoint = mission.waypoints[0];
      items.push({
        command: MAV_CMD.NAV_TAKEOFF,
        param1: 0, // Pitch
        param2: 0, // Empty
        param3: 0, // Empty
        param4: 0, // Yaw angle
        x: firstWaypoint.position.lat,
        y: firstWaypoint.position.lng,
        z: firstWaypoint.altitude_m
      });
    }

    // Convert waypoints to MAVLink commands
    mission.waypoints.forEach((waypoint, index) => {
      switch (waypoint.waypoint_type) {
        case WaypointType.Navigation:
        case WaypointType.Survey:
        case WaypointType.DataCollection:
          items.push({
            command: MAV_CMD.NAV_WAYPOINT,
            param1: 0, // Hold time
            param2: 0, // Accept radius
            param3: 0, // Pass radius
            param4: 0, // Yaw
            x: waypoint.position.lat,
            y: waypoint.position.lng,
            z: waypoint.altitude_m
          });
          break;

        case WaypointType.Landing:
          items.push({
            command: MAV_CMD.NAV_LAND,
            param1: 0, // Abort altitude
            param2: 0, // Precision land mode
            param3: 0, // Empty
            param4: 0, // Yaw angle
            x: waypoint.position.lat,
            y: waypoint.position.lng,
            z: 0 // Land altitude
          });
          break;

        default:
          // Default to waypoint
          items.push({
            command: MAV_CMD.NAV_WAYPOINT,
            param1: 0,
            param2: 0,
            param3: 0,
            param4: 0,
            x: waypoint.position.lat,
            y: waypoint.position.lng,
            z: waypoint.altitude_m
          });
      }

      // Add action commands
      waypoint.actions.forEach(action => {
        switch (action.type) {
          case 'TakePhoto':
            items.push({
              command: MAV_CMD.IMAGE_START_CAPTURE,
              param1: 0, // Camera ID
              param2: 0, // Interval
              param3: 1, // Total images
              param4: 0, // Sequence number
            });
            break;
        }
      });
    });

    // Add landing command if not already present
    const hasLanding = items.some(item => item.command === MAV_CMD.NAV_LAND);
    if (!hasLanding && mission.waypoints.length > 0) {
      const lastWaypoint = mission.waypoints[mission.waypoints.length - 1];
      items.push({
        command: MAV_CMD.NAV_LAND,
        param1: 0,
        param2: 0,
        param3: 0,
        param4: 0,
        x: lastWaypoint.position.lat,
        y: lastWaypoint.position.lng,
        z: 0
      });
    }

    return {
      version: 1,
      ground_station: 'AgBot Mission Planner',
      items
    };
  }

  async exportMAVLink(mission: Mission): Promise<string> {
    const mavlinkMission = await this.convertToMAVLink(mission);
    
    // Convert to MAVLink waypoint file format
    let output = 'QGC WPL 110\n';
    mavlinkMission.items.forEach((item, index) => {
      const line = [
        index, // seq
        1, // current
        0, // autocontinue
        item.command,
        item.param1 || 0,
        item.param2 || 0,
        item.param3 || 0,
        item.param4 || 0,
        item.x || 0,
        item.y || 0,
        item.z || 0,
        1 // mission type
      ].join('\t');
      output += line + '\n';
    });

    return output;
  }
}
