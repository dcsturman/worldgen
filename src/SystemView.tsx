import React from 'react';
import { System, GasGiant, World, StarType, StarSize} from './worldgen';
import WorldTable from './WorldTable';

// type ComponentProps = React.PropsWithChildren<{
//     prop1: string;
//     prop2: number;
// }>

// const Component: React.FunctionComponent<ComponentProps> = ({ prop1, prop2, children }) => (
//     <div>

//     </div>
// )

type SystemViewProps = {
  system: System;
};

const SystemView: React.FunctionComponent<SystemViewProps> = ({ system }) => {
  let secondary_preamble = <></>;
  if (system.secondary !== null) {
    if (system.secondary.orbit === 0) {
      secondary_preamble = <>&nbsp; It has a secondary contact star {system.secondary.name}.&nbsp;<SystemPreamble system={system.secondary} /></>;
    } else {
      secondary_preamble = <>&nbsp; It has a secondary star {system.secondary.name} at orbit {system.secondary.orbit}.&nbsp;<SystemPreamble system={system.secondary} /></>;
    }
  }

  let tertiary_preamble = <></>;
  if (system.tertiary !== null) {
    if (system.tertiary.orbit === 0) {
      tertiary_preamble = <>It has a tertiary contact star {system.tertiary.name}.&nbsp;<SystemPreamble system={system.tertiary} /></>;
    } else {
      tertiary_preamble = <>It has a tertiary star {system.tertiary.name} at orbit {system.tertiary.orbit}.&nbsp;<SystemPreamble system={system.tertiary} /></>;
    }
  }

  let num_stars = count_stars(system) - 1
  let num_gas_giants = system.orbits.filter(
    (x) => x instanceof GasGiant
  ).length;
  let num_planetoids = system.orbits.filter(
    (x) => x instanceof World && x.name === "Planetoid Belt"
  ).length;
  let num_satellites =
    system.orbits
      .filter((x) => x instanceof World)
      .map((x) => (x as World).num_satellites())
      .reduce((acc, x) => acc + x, 0) +
    system.orbits
      .filter((x) => x instanceof GasGiant)
      .map((x) => (x as GasGiant).num_satellites())
      .reduce((acc, x) => acc + x, 0);

  return <div>
    <SystemPreamble system={system} />
    {secondary_preamble}
    {tertiary_preamble}
    <br />
    <span>
      {system.name && num_gas_giants + num_stars + num_planetoids + num_satellites > 0 ? system.name + " " : ""} has {num_stars >= 2 ? num_stars + " stars, " : num_stars === 1 ? "1 star, " : ""}
      {num_gas_giants >= 2 ? num_gas_giants + " gas giants, " : num_gas_giants === 1 ? "1 gas giant, " : ""}
      {num_planetoids >= 2 ? num_planetoids + " planetoids, " : num_planetoids === 1 ? "1 planetoid, " : ""}
      {num_satellites >= 2 ? num_satellites + " satellites." : num_satellites === 1 ? "1 satellite." : ""}
    </span>
    <br />
    <br />
    <SystemMain system={system} is_companion={false}/>
  </div>;
  };


const SystemPreamble: React.FunctionComponent<SystemViewProps> = ({ system }) => (
  <span>
    <b>{system.name}</b> is a {StarType[system.star.star_type]}{system.star.subtype} {StarSize[system.star.size]} star.
  </span>
);

type SystemMainProps = {
  system: System;
  is_companion: boolean;
};

const SystemMain: React.FunctionComponent<SystemMainProps> = ({ system, is_companion }) => (
  <div>
    <WorldTable key={system.name + "-table"} primary={system} is_companion={is_companion} worlds={system.orbits.filter((x) => x !== null)} />
    <br />
    {system.secondary !== null ? <>{system.name}'s secondary star {system.secondary.name}:<br /><SystemMain system={system.secondary} is_companion={true}/><br /></> : <></>}
    {system.tertiary !== null ? <>{system.name}'s tertiary star {system.tertiary.name}:<br /><SystemMain system={system.tertiary} is_companion={true}/><br /></> : <></>}
  </div>
);

function count_stars(system: System): number {
  let count = 1;
  if (system.secondary != null) {
    count += count_stars(system.secondary);
  }
  if (system.tertiary != null) {
    count += count_stars(system.tertiary);
  }
  return count;
}
export default SystemView;
