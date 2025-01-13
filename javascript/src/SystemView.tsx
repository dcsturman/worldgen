import React from "react";
import { System, GasGiant, World, StarType, StarSize, FAR_ORBIT, getHabitable } from "./worldgen";
import WorldTable from "./WorldTable";

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
  ref: React.Ref<HTMLDivElement>;
};

function habitableClause(system: System): string {
  let habitable = getHabitable(system);
  return habitable > -1 && habitable <= system.max_orbits ? ` with a habitable zone at orbit ${habitable}` : "";
}

const SystemView: React.FunctionComponent<SystemViewProps> = ({
  system,
  ref,
}) => {
  return (
    <div className="output-region" ref={ref}>
      <h2>The {system.main_world?.name} System</h2>
      The primary star of the {system.main_world?.name} system is <b>{system.name}</b>, a {StarType[system.star.star_type]}
      {system.star.subtype} {StarSize[system.star.size]} star{habitableClause(system)}.
      <SystemPreamble system={system} />
      <br />
      <br />
      <SystemMain system={system} is_companion={false} />
    </div>
  );
};

type SystemPreambleProps = {
  system: System;
};

const SystemPreamble: React.FunctionComponent<SystemPreambleProps> = ({
  system,
}) => {
  let secondary_lead = <></>;
  if (system.secondary !== null) {
    let secondary = system.secondary;
    if (system.secondary.orbit === 0) {
      secondary_lead = (
        <>
          &nbsp; It has a secondary contact star {secondary.name}, which is a {StarType[secondary.star.star_type]}
          {secondary.star.subtype} {StarSize[secondary.star.size]} star.&nbsp;
        </>
      );
    } else if (system.secondary.orbit < FAR_ORBIT) {
      secondary_lead = (
        <>
          &nbsp; It has a secondary star {secondary.name} at orbit{" "}
          {secondary.orbit}, which is a {StarType[secondary.star.star_type]}
          {secondary.star.subtype} {StarSize[secondary.star.size]} star{habitableClause(secondary)}.&nbsp;
        </>
      );
    } else {
      secondary_lead = (
        <>
          &nbsp; It has a secondary star {secondary.name} in far orbit, which is a {StarType[secondary.star.star_type]}
          {secondary.star.subtype} {StarSize[secondary.star.size]} star{habitableClause(secondary)}.
          &nbsp;
        </>
      );
    }
  }

  let tertiary_lead = <></>;
  if (system.tertiary !== null) {
    let tertiary = system.tertiary;
    if (tertiary.orbit === 0) {
      tertiary_lead = (
        <>
          It has a tertiary contact star {system.tertiary.name}, which is a {StarType[tertiary.star.star_type]}
          {tertiary.star.subtype} {StarSize[tertiary.star.size]} star.&nbsp;
        </>
      );
    } else if (tertiary.orbit < FAR_ORBIT) {
      tertiary_lead = (
        <>
          It has a tertiary star {tertiary.name} at orbit{" "}
          {tertiary.orbit}, which is a {StarType[tertiary.star.star_type]}
          {tertiary.star.subtype} {StarSize[tertiary.star.size]} star{habitableClause(tertiary)}.&nbsp;
        </>
      );
    } else {
      tertiary_lead = (
        <>
          It has a tertiary star {tertiary.name} in far orbit, which is a {StarType[tertiary.star.star_type]}
          {tertiary.star.subtype} {StarSize[tertiary.star.size]} star{habitableClause(tertiary)}.&nbsp;
        </>
      );
    }
  }

  let num_stars = count_stars(system) - 1;
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

  return (
    <span>
      <span>
        &nbsp;
        {system.name &&
        num_gas_giants + num_stars + num_planetoids + num_satellites > 0
          ? system.name +
            " has " +
            [
              num_stars >= 2
                ? num_stars + " stars"
                : num_stars === 1
                ? "1 star"
                : "",
              num_gas_giants >= 2
                ? num_gas_giants + " gas giants"
                : num_gas_giants === 1
                ? "1 gas giant"
                : "",
              num_planetoids >= 2
                ? num_planetoids + " planetoids"
                : num_planetoids === 1
                ? "1 planetoid"
                : "",
              num_satellites >= 2
                ? num_satellites + " satellites"
                : num_satellites === 1
                ? "1 satellite"
                : "",
            ]
              .filter((x) => x.length > 0)
              .join(", ") +
            "."
          : ""}
      </span>
      {system.secondary !== null ? (
        <span>
          {secondary_lead} <SystemPreamble system={system.secondary} />
        </span>
      ) : (
        <></>
      )}
      {system.tertiary !== null ? (
        <span>
          &nbsp;{tertiary_lead} <SystemPreamble system={system.tertiary} />
        </span>
      ) : (
        <></>
      )}
    </span>
  );
};

type SystemMainProps = {
  system: System;
  is_companion: boolean;
};

const SystemMain: React.FunctionComponent<SystemMainProps> = ({
  system,
  is_companion,
}) => (
  <div>
    <WorldTable
      key={system.name + "-table"}
      primary={system}
      worlds={system.orbits.filter((x) => x !== null)}
    />
    <br />
    {system.secondary !== null ? (
      <>
        {system.name}'s secondary star {system.secondary.name}:<br />
        <SystemMain system={system.secondary} is_companion={true} />
        <br />
      </>
    ) : (
      <></>
    )}
    {system.tertiary !== null ? (
      <>
        {system.name}'s tertiary star {system.tertiary.name}:<br />
        <SystemMain system={system.tertiary} is_companion={true} />
        <br />
      </>
    ) : (
      <></>
    )}
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
