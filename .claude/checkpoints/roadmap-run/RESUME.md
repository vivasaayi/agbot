# Resume — world-sim-run-1

- Branch: simulator-enhancements; last commit d424941 (M5 roads+renderer); prior ce87ab9.
- ALL plan milestones M0-M4 complete and committed:
  M0 config core | M1 terrain_engine + worldgen(NYC) + GL4.1 renderer + Manhattan demo |
  M2 vehicles + nav pipeline | M3 Cessna 6-DOF + Dubins + flythrough |
  M4 ONNX mono-depth (RMSE 2.38 m vs DEM) + hybrid-A* + MPPI + EKF + ONNX/classical segmentation.
- Validation: 14/14 ctest suites green, ONNX active, zero warnings.
- Fresh-clone prerequisites (gitignored): fetch_nyc_buildings.sh, fetch_depth_model.sh,
  fetch_seg_model.sh, Terrarium tiles z13 x2411-2412 y3079-3080.
- User's dirty files untouched: src/MissionLoader.cpp, src/macos_opengl_viewer.mm, tests/simulation_tests.cpp.
- M5 DONE: OSM road import + welded street graph + road_graph planner (real route 5090 m);
  AGBSCN02 textured meshes, basemap draped over terrain, 4x MSAA, sky gradient.
- Fresh-clone also needs: fetch_osm_roads.sh; OSM basemap tiles z15 x9646-9650 y12316-12321.
- Next (beyond plan): dynamic obstacles + tracking, CDLOD streaming terrain, VIO/LIO SLAM.
