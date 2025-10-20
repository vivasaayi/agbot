import { Mission } from "../types/mission";

export interface MissionStatus {
  mission_id: string;
  status: "preparing" | "deploying" | "active" | "completed" | "failed";
  message?: string;
}

export interface DroneTelemetry {
  position: {
    latitude: number;
    longitude: number;
    altitude: number;
  };
  heading: number;
  speed: number;
  battery_level: number;
  timestamp: string;
}

export class WebSocketService {
  private ws: WebSocket | null = null;
  private connectionStatusCallback?: (status: "connected" | "disconnected" | "connecting") => void;
  private missionStatusCallback?: (status: MissionStatus) => void;
  private telemetryCallback?: (telemetry: DroneTelemetry) => void;

  connect(url: string): void {
    if (this.ws) {
      this.ws.close();
    }

    this.connectionStatusCallback?.("connecting");
    
    this.ws = new WebSocket(url);

    this.ws.onopen = () => {
      console.log("Connected to mission control server");
      this.connectionStatusCallback?.("connected");
    };

    this.ws.onclose = () => {
      console.log("Disconnected from mission control server");
      this.connectionStatusCallback?.("disconnected");
    };

    this.ws.onerror = (error) => {
      console.error("WebSocket error:", error);
    };

    this.ws.onmessage = (event) => {
      try {
        const message = JSON.parse(event.data);
        
        switch (message.type) {
          case "mission_status":
            this.missionStatusCallback?.(message.data);
            break;
          case "drone_telemetry":
            this.telemetryCallback?.(message.data);
            break;
          default:
            console.log("Unknown message type:", message.type);
        }
      } catch (error) {
        console.error("Error parsing WebSocket message:", error);
      }
    };
  }

  disconnect(): void {
    if (this.ws) {
      this.ws.close();
      this.ws = null;
    }
  }

  deployMission(mission: Mission): Promise<{ success: boolean; message?: string }> {
    return new Promise((resolve, reject) => {
      if (!this.ws || this.ws.readyState !== WebSocket.OPEN) {
        reject(new Error("WebSocket not connected"));
        return;
      }

      const message = {
        type: "deploy_mission",
        data: mission
      };

      this.ws.send(JSON.stringify(message));
      
      resolve({ success: true, message: "Mission deployment initiated" });
    });
  }

  deployMAVLinkMission(mavlinkData: string): Promise<{ success: boolean; message?: string }> {
    return new Promise((resolve, reject) => {
      if (!this.ws || this.ws.readyState !== WebSocket.OPEN) {
        reject(new Error("WebSocket not connected"));
        return;
      }

      const message = {
        type: "deploy_mavlink_mission",
        data: { mavlink_data: mavlinkData }
      };

      this.ws.send(JSON.stringify(message));
      
      resolve({ success: true, message: "MAVLink mission deployment initiated" });
    });
  }

  onConnectionStatus(callback: (status: "connected" | "disconnected" | "connecting") => void): void {
    this.connectionStatusCallback = callback;
  }

  onMissionStatus(callback: (status: MissionStatus) => void): void {
    this.missionStatusCallback = callback;
  }

  onDroneTelemetry(callback: (telemetry: DroneTelemetry) => void): void {
    this.telemetryCallback = callback;
  }

  removeAllListeners(): void {
    this.connectionStatusCallback = undefined;
    this.missionStatusCallback = undefined;
    this.telemetryCallback = undefined;
  }
}
