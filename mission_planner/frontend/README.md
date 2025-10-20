# AgBot Mission Planner Frontend

A React+TypeScript frontend for the AgBot drone mission planning system with interactive map interface.

## Features

- 🗺️ **Interactive Map Interface** - Google Maps integration with satellite imagery
- ✈️ **Flight Path Drawing** - Click-to-add waypoints and draw flight paths
- 📡 **Real-time Communication** - WebSocket connection to mission control
- 🔄 **MAVLink Support** - Convert missions to MAVLink format for drone deployment
- 🎛️ **Mission Management** - Save, load, and optimize missions
- 🌤️ **Weather Integration** - Check flight conditions before deployment

## Setup

### Prerequisites
- Node.js 16+ 
- Google Maps API key (for map functionality)

### Installation

1. Install dependencies:
```bash
npm install
```

2. Configure Google Maps API key:
   - Get a Google Maps API key with Maps JavaScript API and Drawing Library enabled
   - Replace `YOUR_GOOGLE_MAPS_API_KEY` in `public/index.html` with your actual key

3. Configure backend connection:
   - The frontend expects the backend API at `http://localhost:3000`
   - WebSocket connection at `ws://localhost:3000/ws`

### Running

Start the development server:
```bash
npm start
```

The app will open at `http://localhost:3001` (port 3001 to avoid conflict with backend on 3000).

## Usage

### Creating a Mission

1. Click "New Mission" to start a new mission
2. Select "Waypoint" tool and click "Start Drawing"
3. Click on the map to add waypoints
4. Waypoints will be connected automatically to show flight path
5. Click on waypoint markers to see details or remove them
6. Save the mission when complete

### Deploying a Mission

1. Ensure WebSocket connection is active (green status indicator)
2. Click "Deploy Mission" to send to drone system
3. Mission will be converted to MAVLink format automatically
4. Monitor mission status in real-time

### Drawing Tools

- **Waypoint**: Click to add individual waypoints
- **Survey Area**: Draw polygons for area coverage missions  
- **Flight Path**: Draw custom flight paths

### Keyboard Shortcuts

- `Ctrl+N` - New Mission
- `Ctrl+S` - Save Mission  
- `Esc` - Stop Drawing Mode
- `Del` - Delete Selected Items

## Architecture

```
src/
├── components/
│   ├── MissionPlanner.tsx    # Main app component
│   ├── MapComponent.tsx      # Google Maps integration
│   ├── MissionPanel.tsx     # Mission management UI
│   └── ControlsPanel.tsx    # Drawing tools and controls
├── services/
│   ├── missionService.ts    # REST API client
│   └── websocketService.ts  # WebSocket client  
├── types/
│   └── mission.ts          # TypeScript type definitions
└── App.tsx                 # App router
```

## API Integration

The frontend communicates with the backend via:

- **REST API** (`/api/missions/*`) - CRUD operations for missions
- **WebSocket** (`/ws`) - Real-time mission deployment and status updates
- **MAVLink Export** (`/api/missions/{id}/mavlink`) - Convert to drone format

## Development

### Adding New Features

1. Define types in `src/types/mission.ts`
2. Add API methods to `src/services/missionService.ts`  
3. Create React components in `src/components/`
4. Update WebSocket handlers in `src/services/websocketService.ts`

### Google Maps Integration

The map component uses:
- Maps JavaScript API for base map
- Drawing Library for interactive drawing tools
- Custom markers and info windows for waypoints
- Polylines for flight path visualization

### WebSocket Protocol

Messages follow this format:
```typescript
{
  type: "DeployMission" | "MissionStatus" | "DroneTelemetry" | ...,
  // ... message-specific data
}
```

## Troubleshooting

### Map not loading
- Check Google Maps API key is valid and enabled
- Ensure Maps JavaScript API and Drawing Library are enabled in Google Cloud Console

### Backend connection issues  
- Verify backend server is running on port 3000
- Check CORS settings if accessing from different domain
- Ensure WebSocket endpoint is accessible

### TypeScript errors
- Run `npm install` to ensure all dependencies are installed
- Check `tsconfig.json` configuration
- Verify type definitions are up to date

## Future Enhancements

- [ ] Offline map support with cached tiles
- [ ] 3D mission visualization  
- [ ] Advanced mission templates
- [ ] Multi-drone coordination
- [ ] Real-time drone tracking overlay
- [ ] Mission simulation and validation
