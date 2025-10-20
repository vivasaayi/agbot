import React, { useRef, useEffect, useState } from 'react';
import { Mission, Waypoint } from '../types/mission';

interface MapComponentProps {
  mission: Mission | null;
  onWaypointAdd: (lat: number, lng: number, altitude?: number) => void;
  onWaypointRemove: (waypointId: string) => void;
  isDrawing: boolean;
  selectedTool: 'waypoint' | 'area' | 'path';
}

const MapComponent: React.FC<MapComponentProps> = ({
  mission,
  onWaypointAdd,
  onWaypointRemove,
  isDrawing,
  selectedTool
}) => {
  const mapRef = useRef<HTMLDivElement>(null);
  const [map, setMap] = useState<google.maps.Map | null>(null);
  const [markers, setMarkers] = useState<google.maps.Marker[]>([]);
  const [drawingManager, setDrawingManager] = useState<google.maps.drawing.DrawingManager | null>(null);

  // Initialize map
  useEffect(() => {
    if (!mapRef.current || map) return;

    // Default center (can be configurable)
    const defaultCenter = { lat: 40.7128, lng: -74.0060 }; // NYC

    const mapInstance = new google.maps.Map(mapRef.current, {
      zoom: 15,
      center: defaultCenter,
      mapTypeId: google.maps.MapTypeId.SATELLITE,
      mapTypeControl: true,
      mapTypeControlOptions: {
        style: google.maps.MapTypeControlStyle.HORIZONTAL_BAR,
        position: google.maps.ControlPosition.TOP_RIGHT,
        mapTypeIds: [
          google.maps.MapTypeId.SATELLITE,
          google.maps.MapTypeId.HYBRID,
          google.maps.MapTypeId.TERRAIN
        ]
      }
    });

    // Initialize drawing manager
    const drawingMgr = new google.maps.drawing.DrawingManager({
      drawingMode: null,
      drawingControl: true,
      drawingControlOptions: {
        position: google.maps.ControlPosition.TOP_CENTER,
        drawingModes: [
          google.maps.drawing.OverlayType.MARKER,
          google.maps.drawing.OverlayType.POLYGON,
          google.maps.drawing.OverlayType.POLYLINE
        ]
      },
      markerOptions: {
        draggable: true,
        icon: {
          url: 'data:image/svg+xml;charset=UTF-8,' + encodeURIComponent(`
            <svg width="20" height="20" viewBox="0 0 20 20" xmlns="http://www.w3.org/2000/svg">
              <circle cx="10" cy="10" r="8" fill="#4CAF50" stroke="#fff" stroke-width="2"/>
              <text x="10" y="14" text-anchor="middle" fill="white" font-size="10">W</text>
            </svg>
          `),
          scaledSize: new google.maps.Size(20, 20)
        }
      },
      polygonOptions: {
        fillColor: '#4CAF50',
        fillOpacity: 0.3,
        strokeWeight: 2,
        strokeColor: '#4CAF50',
        editable: true
      },
      polylineOptions: {
        strokeColor: '#2196F3',
        strokeWeight: 3,
        editable: true
      }
    });

    drawingMgr.setMap(mapInstance);

    // Handle drawing completed
    drawingMgr.addListener('markercomplete', (marker: google.maps.Marker) => {
      const position = marker.getPosition();
      if (position) {
        onWaypointAdd(position.lat(), position.lng());
      }
    });

    // Handle map clicks for waypoint placement
    mapInstance.addListener('click', (event: google.maps.MapMouseEvent) => {
      if (isDrawing && selectedTool === 'waypoint' && event.latLng) {
        onWaypointAdd(event.latLng.lat(), event.latLng.lng());
      }
    });

    setMap(mapInstance);
    setDrawingManager(drawingMgr);
  }, [mapRef, map, onWaypointAdd, isDrawing, selectedTool]);

  // Update markers when mission changes
  useEffect(() => {
    if (!map || !mission) return;

    // Clear existing markers
    markers.forEach(marker => marker.setMap(null));
    setMarkers([]);

    // Add new markers
    const newMarkers: google.maps.Marker[] = [];
    
    mission.waypoints.forEach((waypoint, index) => {
      const marker = new google.maps.Marker({
        position: { lat: waypoint.position.lat, lng: waypoint.position.lng },
        map: map,
        title: `Waypoint ${index + 1}: ${waypoint.waypoint_type}`,
        label: (index + 1).toString(),
        draggable: true,
        icon: {
          url: 'data:image/svg+xml;charset=UTF-8,' + encodeURIComponent(`
            <svg width="30" height="40" viewBox="0 0 30 40" xmlns="http://www.w3.org/2000/svg">
              <path d="M15 0C6.7 0 0 6.7 0 15c0 8.3 15 25 15 25s15-16.7 15-25C30 6.7 23.3 0 15 0z" fill="#4CAF50"/>
              <circle cx="15" cy="15" r="8" fill="white"/>
              <text x="15" y="19" text-anchor="middle" fill="#4CAF50" font-size="10" font-weight="bold">${index + 1}</text>
            </svg>
          `),
          scaledSize: new google.maps.Size(30, 40),
          anchor: new google.maps.Point(15, 40)
        }
      });

      // Add info window
      const infoWindow = new google.maps.InfoWindow({
        content: `
          <div>
            <h4>Waypoint ${index + 1}</h4>
            <p><strong>Type:</strong> ${waypoint.waypoint_type}</p>
            <p><strong>Altitude:</strong> ${waypoint.altitude_m}m</p>
            <p><strong>Position:</strong> ${waypoint.position.lat.toFixed(6)}, ${waypoint.position.lng.toFixed(6)}</p>
            <button onclick="window.removeWaypoint('${waypoint.id}')" style="background: #f44336; color: white; border: none; padding: 5px 10px; border-radius: 3px; cursor: pointer;">Remove</button>
          </div>
        `
      });

      marker.addListener('click', () => {
        infoWindow.open(map, marker);
      });

      newMarkers.push(marker);
    });

    setMarkers(newMarkers);

    // Expose removeWaypoint function globally for info window buttons
    (window as any).removeWaypoint = (waypointId: string) => {
      onWaypointRemove(waypointId);
    };

    // Draw flight path if we have waypoints
    if (mission.waypoints.length > 1) {
      const flightPath = new google.maps.Polyline({
        path: mission.waypoints.map(wp => ({ lat: wp.position.lat, lng: wp.position.lng })),
        geodesic: true,
        strokeColor: '#2196F3',
        strokeOpacity: 1.0,
        strokeWeight: 3,
        map: map
      });
    }

    // Fit map to show all waypoints
    if (mission.waypoints.length > 0) {
      const bounds = new google.maps.LatLngBounds();
      mission.waypoints.forEach(waypoint => {
        bounds.extend(new google.maps.LatLng(waypoint.position.lat, waypoint.position.lng));
      });
      map.fitBounds(bounds);
    }
  }, [map, mission, markers, onWaypointRemove]);

  // Update drawing mode
  useEffect(() => {
    if (!drawingManager) return;

    if (isDrawing) {
      switch (selectedTool) {
        case 'waypoint':
          drawingManager.setDrawingMode(google.maps.drawing.OverlayType.MARKER);
          break;
        case 'area':
          drawingManager.setDrawingMode(google.maps.drawing.OverlayType.POLYGON);
          break;
        case 'path':
          drawingManager.setDrawingMode(google.maps.drawing.OverlayType.POLYLINE);
          break;
      }
    } else {
      drawingManager.setDrawingMode(null);
    }
  }, [drawingManager, isDrawing, selectedTool]);

  return (
    <div 
      ref={mapRef} 
      className="map-container"
      style={{ height: '100%', width: '100%' }}
    />
  );
};

export default MapComponent;
