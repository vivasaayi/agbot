That is a strong product direction: an agriculture flight simulator, not just a drone viewer.

Think of it as:

Microsoft Flight Simulator
  but instead of airports, airliners, and global scenery,
  the core world is farms, fields, crops, weather, drones, sensors, and missions.
The product should be a virtual farm environment where users can:

fly a drone manually
create autonomous flight paths
test coverage over fields
simulate wind, battery, GPS drift, and no-fly zones
practice takeoff/landing
replay missions
compare route efficiency
later simulate RGB, NDVI, thermal, LiDAR, spraying, crop stress, etc.
The architecture I’d aim for:

C++ Vulkan Simulator
  rendering
  terrain
  drone visuals
  camera systems
  input controls
  mission playback
  debug overlays

Simulation Core
  drone physics
  battery model
  wind/weather
  collision/geofence
  sensors
  autopilot behavior

AgBot Backend
  mission planning
  scheduling
  flight path generation
  telemetry storage
  user/scenario data
  reports/replay
The first real MVP should not be “full farm world.” It should be:

MVP 1: Virtual Drone Sandbox

flat terrain/grid
one drone
manual controls
waypoint path
takeoff/fly/land
telemetry panel
replay recording
MVP 2: Farm Mission Simulator

field polygon
generated terrain
crop rows as visual geometry
wind
battery drain
route coverage score
mission scheduler
MVP 3: Agriculture Sensors

virtual RGB camera
fake NDVI layer
coverage heatmap
altitude/resolution simulation
output mission report
MVP 4: Real-World Farm Digital Twin

import boundary
terrain/elevation
satellite base layer
OSM roads/water/buildings
crop zones
weather scenarios
This path is hard, but coherent. You are no longer just “building a drone app.” You are building a drone operating environment where real drone workflows can be learned, tested, optimized, and eventually transferred to hardware.

I would name the internal direction something like:

AgBot FlightSim
And the first engineering goal:

A C++ Vulkan desktop simulator that loads a mission JSON,
spawns a virtual drone, flies the path, records telemetry,
and replays the mission.
That gives you a clean bridge between the current agbot backend and the future custom simulator.