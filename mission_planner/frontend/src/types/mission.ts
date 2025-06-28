export interface Position {
  lat: number;
  lng: number;
}

export interface Waypoint {
  id: string;
  position: Position;
  altitude_m: number;
  waypoint_type: WaypointType;
  actions: Action[];
  arrival_time: string | null;
  speed_ms: number | null;
  heading_degrees: number | null;
}

export enum WaypointType {
  Takeoff = 'Takeoff',
  Navigation = 'Navigation',
  DataCollection = 'DataCollection',
  Survey = 'Survey',
  Emergency = 'Emergency',
  Landing = 'Landing',
  Hover = 'Hover',
  Custom = 'Custom'
}

export interface Action {
  type: ActionType;
  parameters: Record<string, any>;
}

export enum ActionType {
  TakePhoto = 'TakePhoto',
  StartVideo = 'StartVideo',
  StopVideo = 'StopVideo',
  CollectLidar = 'CollectLidar',
  CollectMultispectral = 'CollectMultispectral',
  Hover = 'Hover',
  SetSpeed = 'SetSpeed',
  Wait = 'Wait',
  Custom = 'Custom'
}

export interface WeatherConstraints {
  max_wind_speed_ms: number;
  max_precipitation_mm: number;
  min_visibility_m: number;
  temperature_range_celsius: [number, number];
}

export interface AreaOfInterest {
  coordinates: Position[];
}

export interface Mission {
  id: string;
  name: string;
  description: string;
  created_at: string;
  updated_at: string;
  waypoints: Waypoint[];
  area_of_interest: AreaOfInterest | null;
  estimated_duration_minutes: number;
  estimated_battery_usage: number;
  weather_constraints: WeatherConstraints;
  metadata: Record<string, string>;
}

export interface MAVLinkCommand {
  command: number;
  param1?: number;
  param2?: number;
  param3?: number;
  param4?: number;
  x?: number;
  y?: number;
  z?: number;
}

export interface MAVLinkMission {
  version: number;
  ground_station: string;
  items: MAVLinkCommand[];
}

// MAVLink command constants
export const MAV_CMD = {
  NAV_TAKEOFF: 22,
  NAV_WAYPOINT: 16,
  NAV_LAND: 21,
  DO_SET_SERVO: 183,
  IMAGE_START_CAPTURE: 2000,
  IMAGE_STOP_CAPTURE: 2001,
} as const;
