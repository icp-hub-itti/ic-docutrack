# Orbit station version
ORBIT_STATION_VERSION:="0.5.0"

[group('dependencies')]
fetch_all_canisters: fetch_orbit_canisters

[group('dependencies')]
fetch_orbit_canisters:
  echo "Fetching Orbit canisters..."
  curl --fail -L -o "{{WASM_DIR}}/orbit-station.wasm.gz" "https://github.com/dfinity/orbit/releases/download/%40orbit%2Fstation-v{{ORBIT_STATION_VERSION}}/station.wasm.gz"
