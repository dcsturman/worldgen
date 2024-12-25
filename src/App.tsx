import React from 'react';
import { generateSystem, World } from './worldgen';
import SystemView from './SystemView';

function App() {
  let main_world = World.from_upp("Main World", "A788899-A", false, true);
  let system = generateSystem(main_world);

  return (
    <div className="App">
      <SystemView system={system} />
    </div>
  );
}

export default App;
