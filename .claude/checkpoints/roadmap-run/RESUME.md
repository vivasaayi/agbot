# Resume — world-sim-run-1

- Run: world-sim-run-1 (plan: ~/.claude/plans/i-want-to-enhance-hashed-biscuit.md)
- Last commit: 0793613 (M3 Cessna + M4a ONNX depth); prior a85506a (M0-M2 world-simulator foundation, 96 files)
- Completed: M0 config core; M1a terrain_engine; M1b worldgen (2199 NYC buildings);
  M1c GL4.1 renderer + .agbscn; M2a vehicles; M2b nav pipeline; integration demo.
- Validation: 10/10 ctest green; agbot_world_demo --check green; Manhattan render verified.
- Data prerequisites on fresh clone (gitignored): worldgen/tools/fetch_nyc_buildings.sh
  and Terrarium tiles z13 x2411-2412 y3079-3080 into flight_sim_cpp/out/elevation_tiles/.
- User's pre-existing dirty files left untouched: src/MissionLoader.cpp,
  src/macos_opengl_viewer.mm, tests/simulation_tests.cpp.
- M3 DONE: FixedWingModel (C172 6-DOF) + autopilot + Dubins-airplane planner.
- M4a DONE: real ONNX mono-depth (DA-V2-Small, DEM-anchored, RMSE 2.38 m).
- Next action: remaining M4 — hybrid-A*, MPPI, ONNX segmentation extractors, EKF/SLAM.
