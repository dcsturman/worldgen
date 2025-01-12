import { useRef, useState } from "react";
import { generateSystem, World, System } from "./worldgen";
import SystemView from "./SystemView";
import { useReactToPrint } from "react-to-print";

const DEBUG = false;
const INITIAL_UPP = "A788899-A";
const INITIAL_NAME = "Main World";

function App() {
  const contentRef = useRef<HTMLDivElement>(null);
  const reactToPrintFn = useReactToPrint({ contentRef });

  const [system, setSystem] = useState<System | null>(null);

  const handleNewSystem = (newSystem: System) => {
    setSystem(newSystem);
  };

  return (
    <div className="App">
      <h1>Solar System Generator</h1>
      <WorldEntryForm
        onGenerateSystem={handleNewSystem}
        printFn={reactToPrintFn}
        system={system}
      />
      {system !== null ? <SystemView system={system} ref={contentRef} />: <></>}
    </div>
  );
}

type WorldEntryProps = {
  onGenerateSystem: (system: System) => void;
  printFn: () => void;
  system: System | null;
};

const WorldEntryForm: React.FunctionComponent<WorldEntryProps> = ({
  onGenerateSystem,
  printFn,
  system,
}) => {
  const [worldName, setWorldName] = useState(INITIAL_NAME);
  const [upp, setUpp] = useState(INITIAL_UPP);

  const handleSubmit = (event: React.FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    const newWorld = World.from_upp(worldName, upp, false, true);
    const newSystem = generateSystem(newWorld);
    onGenerateSystem(newSystem);
  };

  return (
    <form onSubmit={handleSubmit} className={"world-entry-form"}>
      <div id="entry-data">
        <div className="world-entry-element">
          <label htmlFor="worldName">World Name:</label>
          <input
            type="text"
            id="worldName"
            value={worldName}
            onChange={(e) => setWorldName(e.target.value)}
            required
          />
        </div>
        <div className="world-entry-element">
          <label htmlFor="upp">UPP:</label>
          <input
            type="text"
            id="upp"
            value={upp}
            onChange={(e) => setUpp(e.target.value)}
            pattern="[A-EX][0-9A-F][0-9A-F][0-9A-F][0-9A-F][0-9A-F][0-9A-F]-[0-9A-F]"
            title="UPP format: SAAAHH-X"
            required
          />
        </div>
      </div>
      <div id="entry-buttons">
        <button className="blue-button" type="submit">
          Generate
        </button>
        <button className="blue-button" type="button" onClick={() => printFn()}>
          Print
        </button>
        {DEBUG && <button className="blue-button" type="button" onClick={() => console.log(JSON.stringify(system, null, 2))}>Dbg</button>}
      </div>
    </form>
  );
};

export default App;
