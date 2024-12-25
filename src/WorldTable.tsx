import React from "react";
import { World, GasGiant, GasGiantSize, System, Empty, StarType, StarSize, FAR_ORBIT } from "./worldgen";

const SHOW_EMPTY: boolean = false;

type WorldTableProps = {
  primary: System;
  worlds: (World | GasGiant | System | Empty)[];
  is_companion: boolean;
};

const WorldTable: React.FunctionComponent<WorldTableProps> = ({ primary, worlds, is_companion }) => (
  <table className="world-table">
    <thead>
      <tr>
        <th>Orbit</th>
        <th></th>
        <th>Name</th>
        <th>UPP</th>
        <th>Remarks</th>
      </tr>
    </thead>
    <tbody>
      <PrimaryRow primary={primary} orbit_name={is_companion ? "Companion" : "Primary"} />
      {worlds.map((world, index) => {
        if (world instanceof GasGiant || world instanceof World ) {
          return <WorldView key={world.name + index} world={world} satellite={false} />;
        } else if (index < primary.max_orbits && world instanceof Empty) {
          return <WorldView key={"empty" + index} world={world} satellite={false} />;
        } else if (world instanceof System) {
          return <PrimaryRow key={world.name + index} primary={world} orbit_name={world.orbit.toString()} />;
        }
      })}
    </tbody>
  </table>
);

type PrimaryRowProps = {
  primary: System;
  orbit_name: string;
}

const PrimaryRow: React.FunctionComponent<PrimaryRowProps> = ({ primary, orbit_name }) => (
  <tr>
    <td>{orbit_name === FAR_ORBIT.toString() ? "Far" : orbit_name}</td>
    <td></td>
    <td>{primary.name}</td>
    <td>{StarType[primary.star.star_type]}{primary.star.subtype}&nbsp;{StarSize[primary.star.size]}</td>
  </tr>
);

type WorldViewProps = {
  world: World | GasGiant | Empty;
  satellite: boolean;
};

const WorldView: React.FunctionComponent<WorldViewProps> = ({
  world,
  satellite,
}) => {
  if (!SHOW_EMPTY && world instanceof Empty) {
    return <></>;
  }

  return <>
    <tr>
      {satellite && <td></td>}
      <td>{world.orbit}</td>
      {!satellite && <td></td>}
      <td>{world instanceof Empty ? "Empty" : world.name}</td>
      <td>{world instanceof World ? world.to_upp() :
       world instanceof GasGiant && world.size === GasGiantSize.Small ? "Small GG" :
       world instanceof GasGiant && world.size === GasGiantSize.Large ? "Large GG" :
       ""}</td>
      <td>{world instanceof World ? world.facilities_string() : ""}</td>
    </tr>
    {!(world instanceof Empty) && world.satellites.length > 0 &&
      world.satellites.map((satellite: World | GasGiant, index: number) => (
        (satellite instanceof World || satellite instanceof GasGiant) && <WorldView key={satellite.name+index} world={satellite} satellite={true} />
      ))}
  </>;
};

export default WorldTable;
