# Star System Generation

A system for generating Traveller star systems from current astrophysics. Written for Mongoose Traveller 2nd Edition; usable with most versions of Traveller.

Sep 25, 2026 · @Dan

## 1. How to use these rules

This is a system for generating the star systems of Traveller: the stars, the orbits, what lies in each, and how a ship gets around and refuels once it is there. It is based on the current understanding of how stars and planets form and behave, simplified until it can be rolled with dice and paper. It is written for Mongoose Traveller 2nd Edition and uses its world codes, but it works with most versions of Traveller, since all of them describe worlds the same way.¹

**Built to work with what already exists.** These rules can be used, and are meant to be used, when some of the system is already known: the main world's UWP, the stars and gas-giant count listed on Travellermap or the Traveller Wiki, a system write-up in a sourcebook, or anything the referee has already established in play. Every step says what to do when the fact it would roll is already on record, and the rest of the system is generated to fit around it. The point is to fill in the detail a game needs without contradicting published material or the referee's own canon. That is why the sections that follow keep asking whether something is already known; the answer usually is.

**Dice.** 1D is one six-sided die, 2D is two added together, D3 is one die halved and rounded up, D66 is two dice read as tens and units. No other dice are used. A DM is added to the roll before reading the table. Where a step says "roll again on the rare table", roll a second time on the table named; this is how the rules produce one-in-a-hundred results with six-sided dice.

**Distances.** Every real distance in these rules is in millions of kilometres, written **Mkm**. Orbits, however, are placed and compared in **habitable distances**, written **HD**. One HD is the distance from a given star at which a world receives the same light that Earth receives from the Sun. It is a different real distance for every star: for the Sun, 1 HD is 149.6 Mkm; for a small red dwarf, 1 HD may be 5 Mkm. The point of it is that a position of 1.0 HD means the same warmth around any star, 2.7 HD is the snow line around any star, and so on, so a single set of orbit tables serves every star. Table 5 gives each star's HD in Mkm, and multiplying a position by that number gives the real distance. The astronomical unit, 149.6 Mkm, is deliberately not used for orbits in these rules, because the same AU is warm around one star and frozen around another; it appears only in this paragraph. Travel times are in days at thrust 1 (accelerate to the midpoint, turn over, decelerate); divide by 1.4 for thrust 2, by 1.7 for thrust 3, by 2 for thrust 4.

**Describing a star.** A star is written like **G2 V**. The letter is the **spectral class**, which is its colour and temperature: O, B, A, F, G, K, M from hottest and bluest to coolest and reddest. The digit is the **subtype**, a finer step within the class, 0 hottest to 9 coolest. The Roman numeral is the **luminosity class**, which is the star's stage of life: **V** is the main sequence, the long steady middle age in which a star spends nine tenths of its life and in which almost every star you will roll is found; **VI** is a subdwarf, an old and slightly dim main-sequence star; **III** is a giant, a star near the end of its life, swollen and bright; **II** and **I** are bright giants and supergiants; **D** is a white dwarf, the burnt-out core left after a star has died; **BD** is a brown dwarf, an object too small to become a star at all. The Sun is G2 V.

**Two procedures.** The **quick procedure** gives the stars, the orbits, what sits in each, the main world's place and the refuelling picture. The **full procedure** adds a few rolls per world you care about (temperature, rotation, tilt, eccentricity). Roll the full procedure for worlds the players will land on and leave the rest quick.

**Starting with or without a main world.** Most systems in Charted Space already have a main world with a Universal World Profile, usually with the stars and the number of gas giants and belts as well, from Travellermap, the Traveller Wiki or a sourcebook. For a new system, generate the main world first with the world creation rules in the Core Rulebook, then come here. Either way these rules take the main world as given, place it where its atmosphere and water make sense, and build the rest of the system around it. A system may also be generated with no main world at all, for an uninhabited system or a scout survey; the rules say where that path differs. Anything already published or already rolled is fixed: never re-roll it. If published facts cannot be made physically consistent with each other, keep the facts and note the oddity; Section 13 says how.

**Reading order.** Section 2 is the checklist. Sections 3 to 10 are the steps in order, each with its tables. Section 11 is reference material for play. Section 12 walks through three systems in full. Section 13 holds the optional rules and the notes on where the numbers come from. Tables are numbered through the document and referred to by number.

¹ *Traveller is a registered trademark of Far Future Enterprises. Mongoose Traveller 2nd Edition is published by Mongoose Publishing Ltd. This document is an independent work and is not endorsed by or affiliated with either. Refuelling rates in Section 11 are summarised from the Drinaxian Companion, published by Mongoose Publishing.*

## 2. The procedure at a glance

Steps marked **Q** belong to the quick procedure; **F** steps are added by the full procedure. "Known" means the fact is already published or rolled for this system and is not rolled again.

**Table 1: Procedure**

| Step | Roll or lookup | Section |
| --- | --- | --- |
| 1 Q. Number of stars | Table 2; known if the stars are listed | Section 3.1 |
| 2 Q. Primary star: spectral class, subtype, luminosity class | Tables 3 and 4; known if listed | Sections 3.2, 3.3 |
| 3 Q. Star data | Table 5 (Table 6 for stars that are not class V) | Section 3.4 |
| 4 Q. Companion stars: mass and separation | Tables 7, 8, 9; check which star hosts the main world | Section 3.5 |
| 5 Q. Main world position | Table 10, from its UWP | Section 4.1 |
| 6 Q. Number of orbits | 2D − 2 | Section 4.2 |
| 7 Q. Orbit positions | Table 11 and Table 12, outward from the first orbit or in both directions from the main world; cross out orbits in a companion's gap | Sections 4.3, 4.4 |
| 8 Q. Orbit zones | Table 13 | Section 4.5 |
| 9 Q. Giant planets: presence, number, kind, placement | Tables 15 to 18; known if the count is listed | Sections 5.1 to 5.4 |
| 10 Q. Ice | Table 19 | Section 5.5 |
| 11 Q. Refuelling line | Table 20 | Section 5.6 |
| 12 Q. What fills each remaining orbit | Tables 21, 22 | Section 6 |
| 13 Q. Each world: size, composition, gravity, atmosphere, hydrographics | Table 23 | Sections 7.1 to 7.4 |
| 14 F. Temperature | Tables 24 to 27 | Section 7.5 |
| 15 Q. Moons and rings: number, size, orbits | Tables 28 to 30 | Sections 8.1 to 8.3 |
| 16 F. Moon day, tidal heating | Table 31 | Sections 8.4, 8.5 |
| 17 F. Eccentricity; world rotation: tidal lock and day length | Tables 32, 33 | Sections 9.1, 9.2 |
| 18 F. Axial tilt, year | Tables 34, 35 | Sections 9.3, 9.4 |
| 19 Q. Settlement of the other worlds: population, government, law, facilities, tech level, spaceport | Tables 36 to 39 | Section 10 |
| 20 Q. Record | Position, Mkm, days, zone, world codes, extended profile, notes | Section 7.6 |

A quick system with one star and six orbits takes about forty rolls. The full procedure adds three or four per world you care about.

## 3. Stars

A system has one, two or three stars. Generate the primary first, then any companion stars. If the stars are already listed for the system (Travellermap and the Traveller Wiki list them for most of Charted Space, in the form **G2 V M9 V M6 V**), skip Sections 3.1 to 3.3: the first star listed is the primary, the rest are companions, and you go straight to Table 5 and then to Section 3.5 for the companions' separations.

### 3.1 Number of stars

**Table 2: Number of stars** (DM −1 if the primary is spectral class M)

| 2D | Stars |
| --- | --- |
| 7 or less | One |
| 8 to 10 | Two: the primary and a companion |
| 11 or more | Three: the primary and two companions |

About two systems in five have a companion star, which matches the real neighbourhood of the Sun.

### 3.2 Primary star: spectral class and subtype

Roll 2D on the column that applies. Use the **habitable main world** column when the system is being built around a main world with atmosphere 4 to 9 and hydrographics 1 or more; such a world has had time to gain air and water, which rules out the short-lived hot stars.

**Table 3: Primary spectral class**

| 2D | Habitable main world | Otherwise |
| --- | --- | --- |
| 2 | M | M |
| 3 | M | M |
| 4 | M | M |
| 5 | M | M |
| 6 | K | M |
| 7 | K | M |
| 8 | G | M |
| 9 | G | K |
| 10 | F | G |
| 11 | F | F |
| 12 | A | Rare: roll 1D. 1 to 4 A, 5 B, 6 O (treat O as B0) |

**Subtype**: roll 2D and subtract 2; a result of 10 is 9.

### 3.3 Primary star: luminosity class

If the system is being built around a habitable main world (atmosphere 4 to 9, hydrographics 1 or more), the primary is class V without a roll: a world needs billions of steady years to gain air and water, and only a main-sequence star provides them. If the star is already listed for the system, use it as listed even if it is not class V; Section 13 says what to do when a listed star and a listed world do not fit. Otherwise roll 2D.

**Table 4: Primary luminosity class**

| 2D | Class | Meaning |
| --- | --- | --- |
| 2 | D | White dwarf: the burnt-out core of a former star. Roll spectral class and subtype again to describe what it once was; anything inside 300 Mkm was destroyed when it swelled |
| 3 | III | Giant: a star near the end of its life, swollen and bright. Any inner worlds are scorched or gone |
| 4 | VI | Subdwarf: an old, metal-poor main-sequence star, a little dimmer than a class V of the same spectral class |
| 5 to 12 | V | Main sequence, the normal state of a star for most of its life |

Spectral classes O and B with a luminosity roll of 2 to 4 are supergiants (class I) instead of the listed result.

### 3.4 Star data

Look up the primary and each companion in Table 5. Every later step uses one or two numbers from the row, so copy them onto the record. Table 5 covers luminosity class V, which is nearly every star; for any other luminosity class, take the class V row for the same spectral class and subtype and change it as Table 6 says. The columns, in order:

- **Temp** is the star's surface temperature in kelvin, which sets its colour: above 7,500 white to blue-white, 5,000 to 7,500 yellow-white to yellow, 4,000 to 5,000 orange, below 4,000 red.
- **Light** is the star's brightness with the Sun as 1.
- **Mass** is the star's mass with the Sun as 1. Section 3.5 and Section 9.4 use it.
- **1 HD** is what one habitable distance comes to around this star, in Mkm: the real distance at which a world is as warm as Earth. Multiply it by an orbit's position to get the orbit's real distance.
- **Days to 1 HD** is the travel time at thrust 1 from the star's neighbourhood to an orbit at position 1.0. For other orbits multiply by the travel multiplier in Table 14.
- **Shadow** is the star's jump shadow, its 100-diameter limit, in Mkm. A ship cannot jump from inside it and normally arrives at its edge. For a red dwarf it is bigger than the whole habitable zone, so ships arriving at a red dwarf's garden world appear outside the shadow and fly in.
- **Inner orbit** is the closest position at which an orbit can exist. Any orbit rolled closer is moved out to this position.
- **Lock limit** is the position inside which a world becomes tidally locked to its star. Below 0.95 the habitable zone is free; above 1.7 the whole habitable zone is locked; "all" means every orbit in the system is locked.
- **Moon limit** is a figure for the widest stable satellite orbit; Table 30 in Section 8 turns it into radii for a given orbit.

**Table 5: Star data, luminosity class V**

| Star | Temp (K) | Light | Mass | 1 HD (Mkm) | Days to 1 HD | Shadow (Mkm) | Inner orbit | Lock limit | Moon limit |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| B0 V | 31400 | 20893 | 17.50 | 21,624 | 34.4 | 1,030 | 0.05 | 0.01 | 6514 |
| B1 V | 28260 | 10914 | 13.83 | 15,629 | 29.3 | 906 | 0.05 | 0.01 | 5092 |
| B2 V | 25120 | 5702 | 10.93 | 11,296 | 24.9 | 797 | 0.05 | 0.01 | 3980 |
| B3 V | 21980 | 2979 | 8.64 | 8,165 | 21.1 | 701 | 0.05 | 0.02 | 3111 |
| B4 V | 18840 | 1556 | 6.83 | 5,901 | 18.0 | 617 | 0.05 | 0.02 | 2432 |
| B5 V | 15700 | 813 | 5.40 | 4,265 | 15.3 | 543 | 0.05 | 0.02 | 1901 |
| B6 V | 14500 | 429 | 4.50 | 3,097 | 13.0 | 484 | 0.05 | 0.03 | 1467 |
| B7 V | 13300 | 226 | 3.76 | 2,249 | 11.1 | 432 | 0.05 | 0.04 | 1131 |
| B8 V | 12100 | 119 | 3.13 | 1,633 | 9.5 | 385 | 0.05 | 0.05 | 873 |
| B9 V | 10900 | 63 | 2.61 | 1,186 | 8.1 | 343 | 0.05 | 0.07 | 673 |
| A0 V | 9700 | 33 | 2.18 | 861 | 6.9 | 306 | 0.05 | 0.09 | 519 |
| A1 V | 9380 | 27 | 2.11 | 782 | 6.5 | 294 | 0.05 | 0.10 | 477 |
| A2 V | 9050 | 22 | 2.04 | 709 | 6.2 | 283 | 0.05 | 0.11 | 437 |
| A3 V | 8730 | 19 | 1.98 | 644 | 5.9 | 272 | 0.05 | 0.12 | 401 |
| A4 V | 8400 | 15 | 1.91 | 585 | 5.7 | 261 | 0.05 | 0.13 | 368 |
| A5 V | 8080 | 13 | 1.85 | 531 | 5.4 | 251 | 0.05 | 0.14 | 338 |
| A6 V | 7910 | 11 | 1.80 | 487 | 5.2 | 242 | 0.05 | 0.15 | 313 |
| A7 V | 7740 | 9.0 | 1.75 | 448 | 5.0 | 233 | 0.05 | 0.16 | 291 |
| A8 V | 7560 | 7.6 | 1.70 | 411 | 4.7 | 225 | 0.05 | 0.17 | 269 |
| A9 V | 7390 | 6.4 | 1.66 | 378 | 4.5 | 217 | 0.05 | 0.19 | 250 |
| F0 V | 7220 | 5.4 | 1.61 | 347 | 4.4 | 209 | 0.05 | 0.20 | 231 |
| F1 V | 7080 | 4.6 | 1.55 | 322 | 4.2 | 203 | 0.05 | 0.22 | 218 |
| F2 V | 6940 | 4.0 | 1.49 | 299 | 4.0 | 197 | 0.05 | 0.23 | 205 |
| F3 V | 6790 | 3.5 | 1.44 | 278 | 3.9 | 192 | 0.05 | 0.24 | 193 |
| F4 V | 6650 | 3.0 | 1.38 | 258 | 3.8 | 186 | 0.05 | 0.26 | 181 |
| F5 V | 6510 | 2.6 | 1.33 | 240 | 3.6 | 181 | 0.05 | 0.27 | 171 |
| F6 V | 6390 | 2.2 | 1.27 | 223 | 3.5 | 175 | 0.05 | 0.29 | 161 |
| F7 V | 6280 | 1.9 | 1.21 | 208 | 3.4 | 169 | 0.05 | 0.31 | 152 |
| F8 V | 6160 | 1.7 | 1.16 | 194 | 3.3 | 164 | 0.05 | 0.32 | 144 |
| F9 V | 6050 | 1.5 | 1.11 | 180 | 3.1 | 158 | 0.05 | 0.34 | 136 |
| G0 V | 5930 | 1.3 | 1.06 | 168 | 3.0 | 153 | 0.05 | 0.36 | 129 |
| G1 V | 5850 | 1.1 | 1.03 | 158 | 2.9 | 146 | 0.05 | 0.38 | 123 |
| G2 V | 5770 | 1.0 | 1.00 | 150 | 2.9 | 139 | 0.05 | 0.40 | 117 |
| G3 V | 5730 | 0.91 | 0.99 | 143 | 2.8 | 136 | 0.05 | 0.42 | 112 |
| G4 V | 5700 | 0.83 | 0.98 | 136 | 2.7 | 133 | 0.05 | 0.44 | 107 |
| G5 V | 5660 | 0.76 | 0.97 | 130 | 2.7 | 129 | 0.05 | 0.45 | 103 |
| G6 V | 5580 | 0.67 | 0.95 | 123 | 2.6 | 126 | 0.05 | 0.48 | 98 |
| G7 V | 5500 | 0.60 | 0.93 | 116 | 2.5 | 122 | 0.05 | 0.51 | 93 |
| G8 V | 5430 | 0.53 | 0.91 | 109 | 2.4 | 119 | 0.05 | 0.53 | 88 |
| G9 V | 5350 | 0.47 | 0.89 | 103 | 2.4 | 116 | 0.06 | 0.56 | 83 |
| K0 V | 5270 | 0.42 | 0.87 | 97 | 2.3 | 113 | 0.06 | 0.59 | 79 |
| K1 V | 5100 | 0.34 | 0.83 | 88 | 2.2 | 110 | 0.06 | 0.64 | 73 |
| K2 V | 4940 | 0.28 | 0.80 | 80 | 2.1 | 106 | 0.07 | 0.70 | 67 |
| K3 V | 4770 | 0.23 | 0.76 | 72 | 2.0 | 103 | 0.07 | 0.76 | 62 |
| K4 V | 4610 | 0.19 | 0.73 | 66 | 1.9 | 100 | 0.08 | 0.82 | 57 |
| K5 V | 4440 | 0.16 | 0.70 | 60 | 1.8 | 97 | 0.08 | 0.89 | 52 |
| K6 V | 4320 | 0.13 | 0.67 | 55 | 1.7 | 94 | 0.09 | 0.96 | 49 |
| K7 V | 4200 | 0.11 | 0.64 | 50 | 1.7 | 91 | 0.09 | 1.02 | 46 |
| K8 V | 4090 | 0.10 | 0.62 | 46 | 1.6 | 88 | 0.09 | 1.10 | 43 |
| K9 V | 3970 | 0.08 | 0.59 | 43 | 1.5 | 85 | 0.10 | 1.18 | 40 |
| M0 V | 3850 | 0.07 | 0.57 | 39 | 1.5 | 82 | 0.10 | 1.26 | 37 |
| M1 V | 3700 | 0.05 | 0.50 | 32 | 1.3 | 72 | 0.11 | 1.47 | 32 |
| M2 V | 3550 | 0.03 | 0.44 | 27 | 1.2 | 63 | 0.12 | 1.71 | 27 |
| M3 V | 3380 | 0.02 | 0.32 | 18 | 1.0 | 48 | 0.13 | 2.22 | 21 |
| M4 V | 3200 | 0.0072 | 0.23 | 13 | 0.8 | 36 | 0.14 | 2.88 | 16 |
| M5 V | 3050 | 0.0037 | 0.16 | 9.1 | 0.7 | 28 | 0.15 | all | 13 |
| M6 V | 2800 | 0.0011 | 0.10 | 5.0 | 0.5 | 18 | 0.18 | all | 8 |
| M7 V | 2650 | 0.0007 | 0.09 | 4.1 | 0.5 | 17 | 0.20 | all | 7 |
| M8 V | 2500 | 0.0005 | 0.09 | 3.3 | 0.4 | 15 | 0.23 | all | 6 |
| M9 V | 2400 | 0.0003 | 0.08 | 2.4 | 0.4 | 14 | 0.29 | all | 4 |

**Table 6: Other luminosity classes** (changes to the class V row of the same spectral class and subtype)

| Class | Light | Mass | 1 HD in Mkm and days to 1 HD | Jump shadow | Notes |
| --- | --- | --- | --- | --- | --- |
| VI subdwarf | half | 0.9 × | 0.7 × | 0.8 × | Otherwise as class V; innermost orbit and lock limit unchanged |
| III giant, spectral class K or M | 200 × | 1.5 | 14 × | 40 × (about 5,500 Mkm) | Anything inside 400 Mkm has been engulfed; innermost orbit is where that falls in HD |
| III giant, spectral class G or F | 60 × | 2 | 8 × | 15 × |  |
| II bright giant | 1,000 × | 5 | 30 × | 60 × |  |
| I supergiant | 30,000 × | 15 | 170 × | 300 × | Lives a few million years; nothing has had time to become habitable |
| D white dwarf | 0.001 (old) to 0.05 (young); roll 1D, 1 to 2 young, 3 to 6 old | 0.6 | 1 HD is 5 Mkm (old) to 30 Mkm (young); days 0.5 to 1.3 | 1.8 Mkm | Lock limit: everything inside 300 Mkm, and nothing survives there anyway |
| BD brown dwarf | 0.00001 | 0.05 | 0.5 Mkm; it has no useful habitable zone | 14 Mkm | A failed star, warm hydrogen about 80 times Jupiter's mass. A fuel source, though skimming it is a hard, hot pass |

### 3.5 Companion stars

A **companion** is a second or third star bound to the primary. Its **separation** is the average distance between it and the primary, centre to centre. Companions orbit each other in ellipses, so the real distance swings around this figure over the years, but the separation is what the rules use.

For each companion, roll 2D on Table 7 for its mass as a fraction of the primary's, multiply the primary's mass (Table 5) by that fraction, and find the row of Table 5 with the nearest mass: that row is the companion's spectral class and subtype. Companions are luminosity class V unless the rare result says otherwise. This is what real surveys find: nearly all companions are ordinary main-sequence stars, with a few white dwarfs and brown dwarfs among them.

**Table 7: Companion mass**

| 2D | Companion mass | Note |
| --- | --- | --- |
| 2 | 1.0 × primary | A twin |
| 3 | 0.9 × |  |
| 4 | 0.8 × |  |
| 5 | 0.7 × |  |
| 6 | 0.6 × |  |
| 7 | 0.5 × |  |
| 8 | 0.4 × |  |
| 9 | 0.3 × |  |
| 10 | 0.2 × |  |
| 11 | 0.1 × | If this is below 0.08 solar masses, the companion is a brown dwarf |
| 12 | Rare: roll 1D | 1 to 4 brown dwarf; 5 to 6 white dwarf (the companion was once the bigger star and has already died) |

Then roll 2D on Table 8 for the separation. Separations are given in the **primary's HD**, the same unit the orbits use, so that the two limits in the table can be compared with orbit positions directly: worlds can orbit the primary alone out to a third of the separation, and can orbit the pair together beyond three times the separation. Nothing lasts in between. Real distance is separation × the primary's HD in Mkm; travel time is the primary's days to 1 HD × the travel multiplier.

**Table 8: Companion separation** (DM +1 if the primary is spectral class M)

| 2D | Separation (HD) | Travel × | Worlds orbit the primary alone out to | Worlds orbit both stars beyond |
| --- | --- | --- | --- | --- |
| 2 | 0.05 | 0.22 | 0.017 (a contact pair; treat the two as one star for planets) | 0.15 |
| 3 | 0.2 | 0.45 | 0.07 | 0.6 |
| 4 | 0.7 | 0.84 | 0.23 | 2.1 |
| 5 | 2 | 1.4 | 0.67 | 6 |
| 6 | 6 | 2.4 | 2 | 18 |
| 7 | 20 | 4.5 | 6.7 | 60 |
| 8 | 60 | 7.7 | 20 | 180 |
| 9 | 200 | 14 | 67 | 600 |
| 10 | 600 | 24 | 200 | 1,800 |
| 11 | 2,000 | 45 | 670 | 6,000 |
| 12 | 6,000 | 77 | 2,000 | Nothing orbits both |

For a Sun-like primary the rows run from 7.5 Mkm (a pair almost touching, about one binary in thirty) to 900,000 Mkm (seven months apart at thrust 1). Around a red dwarf the same rows are tighter in Mkm, which is also what surveys find.

**Check the habitable zone.** Position 1.0 must fall outside the gap: either below "worlds orbit the primary alone out to" or above "worlds orbit both stars beyond". On rows 4 and 5 it falls inside the gap, and on those rows there can be no habitable world; this is real, and about one binary in five is like that. If the system has no main world, or its main world is not in the Temperate zone, leave the result as rolled. If the system is being built around a habitable main world, the companion must move: roll 1D. On 1 to 3 move it **inward** one row at a time until position 1.0 is above "worlds orbit both stars beyond" (the main world circles both suns, close together in its sky). On 4 to 6 move it **outward** one row at a time until position 1.0 is below "worlds orbit the primary alone out to" (the main world has one sun and a bright distant companion). Note the move on the record. A published main world is never moved; only the companion is.

**Table 9: A third star** (compare its separation with the second star's)

| Third star's separation | Result |
| --- | --- |
| Less than a third of the second star's | Both companions orbit the primary independently, the third star inside the second |
| More than three times the second star's | Both orbit the primary independently, the third star outside the second |
| Between a third and three times | The two companions form a close pair: they orbit each other at one tenth of the smaller separation, and the pair orbits the primary at the larger separation. Use the larger separation's limits for the primary's worlds |

**Which star hosts the main world.** The main world orbits the primary. If the primary cannot host it (a white dwarf, a giant, a brown dwarf) and a class V companion exists, the main world orbits that companion instead; note this on the record. This is often how a garden world listed with a white dwarf turns out to make sense.

**A companion's own worlds.** A companion may have orbits of its own, generated exactly as for the primary from its own row of Table 5, but only inside a third of the separation. Since the companion's HD differs from the primary's, make that one comparison in Mkm: a third of the separation in Mkm, against each of the companion's orbits in Mkm.

## 4. Orbits and zones

An **orbit** is a place where a body can be, whether or not one is there. Orbits are placed by **position** in HD (Section 1), and are numbered on the record from the star outward: orbit 1, orbit 2 and so on. For each orbit the record shows its position, its real distance in Mkm (position × the star's HD in Mkm), its travel time (days to 1 HD × the travel multiplier in Table 14) and its zone.

### 4.1 Placing a known main world

If the system is being built around a known main world, its position comes first and the other orbits are built from it; if there is no main world, skip to Section 4.2. Read the position band from the world's atmosphere and hydrographics in Table 10, roll 1D, and spread the band evenly across the six results: 1 is the inner end of the band, 6 the outer end. Some sourcebooks and wiki system pages state the main world's orbit or distance; if so, convert it to HD (distance in Mkm ÷ the star's HD in Mkm) and use it instead.

**Table 10: Main world position by UWP**

| Main world | Position band (HD) | Notes |
| --- | --- | --- |
| Atmosphere 4 or 5, hydrographics 1+ | 0.85 to 1.00 | Thin air holds little heat, so the world sits close to the inner edge |
| Atmosphere 6 or 7, hydrographics 1+ | 0.90 to 1.15 | Earth-like. With Earth's greenhouse a world is cold beyond 1.15 |
| Atmosphere 8 or 9, hydrographics 1+ | 1.10 to 1.40 | A dense atmosphere keeps a world warm further out |
| Atmosphere 4 to 9, hydrographics 0 | Roll 1D: 1 to 4, 0.60 to 0.85 (a hot desert); 5 to 6, 1.25 to 1.60 (a cold desert) |  |
| Atmosphere 2 or 3 | 0.75 to 0.95 | Very thin air: a Mars with more of it, near the inner edge |
| Atmosphere 0 or 1 | Roll 2D: 2 0.10, 3 0.16, 4 0.25, 5 0.40, 6 0.63, 7 1.0, 8 1.6, 9 2.5, 10 4.0, 11 6.3, 12 10 | Airless worlds can be anywhere; hydrographics above 0 means ice |
| Atmosphere A (exotic) or F (unusual) | 1.00 to 1.30 |  |
| Atmosphere B or C | Roll 1D: 1 to 3, 0.30 to 0.90 (Venus-like); 4 to 6, 2.5 to 3.7 (a cold world kept mild by a thick poisonous atmosphere) |  |
| Atmosphere D (dense, high) | 1.60 to 2.10 | The dense atmosphere does the work; the world sits in the Cold zone |
| Atmosphere E (thin, low) | 0.80 to 1.00 |  |

These bands are the positions at which the temperature rules in Section 7.5 give a Temperate result for that atmosphere, so a main world placed this way comes out livable. The Core Rulebook's world creation includes a temperature step (2D with DMs for atmosphere, giving Frozen to Roasting); it is not part of the UWP and is rolled without reference to the star or the orbit. If you rolled one, set it aside: Section 7.5 works the main world's temperature out from where it actually sits, and that replaces the earlier roll.

### 4.2 Number of orbits

Roll 2D − 2, minimum 1. This is the number of orbits around this star, whether or not each ends up occupied. If the number of bodies in the system is already known (Travellermap gives a count of worlds for many systems), use at least that many orbits. Small stars pack their orbits closer together rather than having fewer of them, so there is no DM for the star.

### 4.3 Building the orbits

**Without a main world.** Roll 2D on Table 11 for the position of orbit 1. Then roll on Table 12 for each further orbit, multiplying each position by the ratio rolled to get the next.

**From a known main world.** The main world's position is one of the orbits. Roll 1D to split the remaining orbits: 1 to 2, one third of them lie inside the main world; 3 to 4, half; 5 to 6, two thirds (round down). For each inward orbit roll on Table 12 and **divide** the previous position by the ratio, working inward from the main world; for each outward orbit roll on Table 12 and **multiply**, working outward.

**Table 11: Position of orbit 1** (HD)

| 2D | Position |  | 2D | Position |
| --- | --- | --- | --- | --- |
| 2 | 0.05 |  | 8 | 0.20 |
| 3 | 0.063 |  | 9 | 0.25 |
| 4 | 0.08 |  | 10 | 0.32 |
| 5 | 0.10 |  | 11 | 0.40 |
| 6 | 0.125 |  | 12 | 0.50 |
| 7 | 0.16 |  |  |  |

Compare the innermost position with the star's **innermost orbit** in Table 5. Any position closer than that is moved out to it, and any inward orbit that would fall below it is not generated. This only matters for the smallest red dwarfs, whose habitable zones sit so close that there is little room inside them.

### 4.4 Spacing and companion gaps

**Table 12: Spacing ratio** (multiply outward, divide inward)

| 2D | Ratio |  | 2D | Ratio |
| --- | --- | --- | --- | --- |
| 2 | 1.25 |  | 8 | 1.90 |
| 3 | 1.35 |  | 9 | 2.05 |
| 4 | 1.45 |  | 10 | 2.25 |
| 5 | 1.55 |  | 11 | 2.50 |
| 6 | 1.65 |  | 12 | 2.80 |
| 7 | 1.75 |  |  |  |

Real systems space their orbits by a factor between about 1.3 and 3, most often near 1.8. Round positions to two significant figures. Stop when the count from Section 4.2 is reached or a position passes 100, whichever comes first. If the count is known and has not been reached when a position passes 100, see **More orbits for a known count** below.

**Companion gaps.** If the system has a companion star, Table 8 gives a gap for it: from "worlds orbit the primary alone out to" up to "worlds orbit both stars beyond". A position that falls in the gap is crossed out as you generate it; it does not count toward the number of orbits, and you keep multiplying from it to find the next. Orbits beyond the gap circle both stars together; treat them as orbits of the primary, and if the companion is of a similar spectral class treat the position as 0.7 × what the arithmetic gives, since two suns warm a world more than one. Orbits inside the gap's inner edge are ordinary orbits of the primary, even when the companion is close: a world at 0.05 with a companion at 0.2 circles its star undisturbed inside the companion's orbit, as planets in real close binaries do. It looks odd on a map and is not.

**More orbits for a known count.** When the number of bodies is known (Section 4.2) and the outward run has passed 100 without reaching it, the system is more tightly packed than Table 12 allows. Split the widest gap between two neighbouring positions: put a new orbit at the geometric mean of the two (multiply them together and take the square root), provided the two are at least 1.56 apart in ratio, so that both halves stay at or above Table 12's minimum of 1.25. For this purpose the edges of a companion's gap count as positions, so the stretch from a gap's outer edge to the next orbit can be split like any other, but a new orbit never goes inside the gap itself. Repeat, always splitting the widest gap left, until every body has an orbit. Only if no gap can be split does an orbit go beyond 100; note it on the record. Real systems with many planets are packed at ratios of 1.3 to 1.6, so a split orbit is the more natural result, and it keeps the extra bodies within reach of the main world rather than months out.

### 4.5 Zones

Read the zone straight off the position. The zone decides what kind of body can occupy the orbit (Section 6) and is the first input to a world's temperature (Section 7).

**Table 13: Orbit zones**

| Zone | Position (HD) | What it means |
| --- | --- | --- |
| Inner | below 0.5 | Scorched. Rock, metal and airless worlds; any atmosphere is thick and hostile, or long gone |
| Hot | 0.5 to 0.95 | Venus to the inner edge of Earth's range. Water survives only as vapour or in cold traps; dense atmospheres run away |
| Temperate | 0.95 to 1.7 | The habitable zone. A world with air and water can be Earth-like here; the outer half needs a dense atmosphere to stay warm |
| Cold | 1.7 to 2.7 | Mars-like. Water is ice; thin atmospheres freeze out |
| Outer | 2.7 and beyond | Past the snow line. Ice is a building material, and giant planets form here |

### 4.6 The position table

Table 14 turns a position into its zone and its travel multiplier; days to an orbit = the star's days to 1 HD × the travel multiplier. Use the nearest row.

**Table 14: Positions**

| Position (HD) | Zone | Travel × |  | Position (HD) | Zone | Travel × |
| --- | --- | --- | --- | --- | --- | --- |
| 0.05 | Inner | 0.22 |  | 1.7 | Cold | 1.30 |
| 0.063 | Inner | 0.25 |  | 2.0 | Cold | 1.41 |
| 0.08 | Inner | 0.28 |  | 2.3 | Cold | 1.52 |
| 0.10 | Inner | 0.32 |  | 2.7 | Outer | 1.64 |
| 0.125 | Inner | 0.35 |  | 3.2 | Outer | 1.79 |
| 0.16 | Inner | 0.40 |  | 4.0 | Outer | 2.00 |
| 0.20 | Inner | 0.45 |  | 5.0 | Outer | 2.24 |
| 0.25 | Inner | 0.50 |  | 6.3 | Outer | 2.51 |
| 0.32 | Inner | 0.57 |  | 8.0 | Outer | 2.83 |
| 0.40 | Inner | 0.63 |  | 10 | Outer | 3.16 |
| 0.50 | Hot | 0.71 |  | 12.5 | Outer | 3.54 |
| 0.63 | Hot | 0.79 |  | 16 | Outer | 4.00 |
| 0.80 | Hot | 0.89 |  | 20 | Outer | 4.47 |
| 0.95 | Temperate | 0.97 |  | 25 | Outer | 5.00 |
| 1.0 | Temperate | 1.00 |  | 32 | Outer | 5.66 |
| 1.1 | Temperate | 1.05 |  | 40 | Outer | 6.32 |
| 1.25 | Temperate | 1.12 |  | 50 | Outer | 7.07 |
| 1.4 | Temperate | 1.18 |  | 63 | Outer | 7.94 |
| 1.6 | Temperate | 1.26 |  | 80 | Outer | 8.94 |
|  |  |  |  | 100 | Outer | 10.0 |

## 5. Giant planets and fuel

This section places the system's **giant planets** and settles its refuelling picture. Section 6 then fills the orbits that remain. Two kinds of giant planet exist. **Gas giants** are Jupiter and Saturn: mostly hydrogen and helium, 100 to 300 times Earth's mass. **Ice giants** are Uranus and Neptune: 15 to 20 Earth masses, with a hot interior of water, ammonia and methane (astronomers call these "ices" whatever their temperature) under a deep hydrogen and helium atmosphere. Both have the gaseous hydrogen envelope a ship skims for fuel; the name says what is inside, not what the ship meets. Where Traveller says "gas giant", as on the sector map, it means either.

### 5.1 Presence

If the number of giant planets is already known for the system (Travellermap and the wiki list it for most of Charted Space), use it and skip Table 15; a listed count of 0 means there are none. Otherwise roll 2D.

**Table 15: Giant planets present**

| 2D | Result |
| --- | --- |
| 9 or less | At least one giant planet |
| 10 or more | None |

This is Traveller's rate, 83%, kept so that refuelling stays easy on ordinary routes. Surveys of nearby stars find a rate nearer 60% once ice giants are counted; Section 13.1 gives that as an option.

### 5.2 Number

**Table 16: Number of giant planets**

| 2D | Giants |
| --- | --- |
| 2 to 3 | 1 |
| 4 to 5 | 2 |
| 6 to 7 | 3 |
| 8 to 11 | 4 |
| 12 | 5 |

Roll the kind of each giant in Section 5.3, then place them one at a time in Section 5.4.

### 5.3 Kind

**Table 17: Kind of giant planet** (roll for each)

| 2D | Kind | Size |
| --- | --- | --- |
| 2 to 7 | Ice giant | 45,000 to 55,000 km. The more common kind |
| 8 to 12 | Gas giant | Roll 1D: 1 to 4 Saturn-class, 60,000 to 100,000 km; 5 to 6 Jupiter-class, 120,000 to 160,000 km |

### 5.4 Placement

**Table 18: Placing the first giant planet** (1D)

| 1D | Orbit |
| --- | --- |
| 1 to 4 | The first free Outer-zone orbit, just past the snow line, where giants form |
| 5 | The first free Cold-zone orbit |
| 6 | A giant that has migrated inward. Roll 1D again: 1 to 5, the first free Temperate or Hot orbit; 6, the first free Inner orbit (a "hot Jupiter", rare, and it has cleared every orbit inside it) |

Place the giants one at a time, in the order rolled. If the zone Table 18 names has no free orbit, the giant takes the first free Cold or Outer orbit instead. Each giant after the first takes the next free Cold or Outer orbit outward from the last one placed, or the first free Cold or Outer orbit if none lies further out. If a giant is to be placed and no Cold or Outer orbit is free, add an orbit for it as in Section 4.4: roll Table 12 and multiply outward from the outermost orbit if the result is 100 or less and clear of any companion star's gap (Table 8); otherwise split the widest gap between two Cold or Outer orbits, putting the new orbit at their geometric mean, provided they are at least 1.56 apart in ratio. If neither is possible, the giant cannot be placed: for a rolled count, drop it; for a known count, put it in the outermost free orbit of any zone and note it on the record. Do this for every giant when the count is known; when the count was rolled, add one orbit at most and drop any giant still unplaced. A giant in the Temperate zone displaces any world there, but it is the natural home of a large habitable moon (Section 8).

### 5.5 Ice bodies

Every system has ice somewhere beyond its snow line: comets, icy moons, and small icy worlds are left over from the way planets form, whether or not a giant planet formed too. What varies is whether that ice is easy to get at. A charted ice belt or a large icy moon is a known place to land and refuel. Scattered ice bodies have to be found first, and a small one yields fuel slowly. Roll 2D on Table 19 for the system.

**Table 19: Ice** (2D)

| 2D | Ice in the system | On the record |
| --- | --- | --- |
| 2 or 3 | Sparse: no charted ice belt and no icy moons of note. Ice exists in scattered small bodies that must be searched for (Section 11.3 gives the search time) and yield fuel at the sparse-ice rate | Fuel: ice, uncharted |
| 4 to 9 | Charted: the outermost orbit beyond the last giant (or the outermost orbit of the system, if there are no giants) holds a belt of ice bodies like the Kuiper belt. If no orbit lies beyond the last giant, or the system's belts are all accounted for by a known belt count, the charted ice is in the moons of the outermost giant, or on the outermost world if there is no giant | Fuel: ice, charted |
| 10 or more | Rich: as above, and every Outer-zone body has surface ice worth landing on | Fuel: ice, charted |

Whatever the roll, any ice-rock world in the Cold or Outer zones (Section 7.2), any world with hydrographics 1 or more in those zones, and any moon of an outer giant is also an ice source; mark each on the record.

An ice belt is a planetoid belt in every sense, so it counts toward the system's number of belts. If the number of belts is already known (Travellermap and the wiki list it), the ice belt is one of them; with a known count of 0 there is no ice belt, and the charted ice is in moons or on a world as the table says.

### 5.6 The refuelling line

With the giant planets placed and the ice known, write the system's refuelling options on the record. Two lines cover it.

- **Transit fuel.** What a ship passing through can use without visiting the main world: it jumps to the source's 100-diameter limit, refuels, and jumps on. Write the best source the system has: **giant planet** (skim, a few hours), **charted ice** (land and melt, most of a day), or **uncharted ice** (search first, then land and melt at the sparse rate; a day or more, and a real chance of coming up dry in the time available). Every system with a star has one of the three. The gas-giant symbol on a sector map means the first.
- **Local fuel.** The travel time at thrust 1 from the main world to the nearest fuel source, read from Table 20 using the fuel source's position (or the main world's, if that is further) and the star's days to 1 HD. Under a week is convenient; under three weeks is reachable on a normal fuel load, since most ships carry four weeks of power-plant fuel; over three weeks means the main world's own ships cannot refuel there without a jump.

The consequence for play is worth saying plainly: no system with a star is a dead end. A ship can always refuel somewhere in it. What the rules decide is how long it takes and how much can go wrong: a giant is a few hours of skimming, charted ice is a landing and a day, uncharted ice is a search and a gamble, and all wilderness fuel is unrefined until the ship's processors have had time to clean it.

**Table 20: Days at thrust 1 from the main world to a fuel source** (columns are the star's days to 1 HD; use the nearest)

| Fuel source position (HD) | 0.5 | 0.8 | 1.2 | 1.8 | 2.4 | 3.0 | 4.0 | 6.0 |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 0.5 | 0.4 | 0.6 | 0.8 | 1.3 | 1.7 | 2.1 | 2.8 | 4.2 |
| 1 | 0.5 | 0.8 | 1.2 | 1.8 | 2.4 | 3.0 | 4.0 | 6.0 |
| 1.5 | 0.6 | 1.0 | 1.5 | 2.2 | 2.9 | 3.7 | 4.9 | 7.3 |
| 2 | 0.7 | 1.1 | 1.7 | 2.5 | 3.4 | 4.2 | 5.7 | 8.5 |
| 3 | 0.9 | 1.4 | 2.1 | 3.1 | 4.2 | 5.2 | 6.9 | 10 |
| 5 | 1.1 | 1.8 | 2.7 | 4.0 | 5.4 | 6.7 | 8.9 | 13 |
| 8 | 1.4 | 2.3 | 3.4 | 5.1 | 6.8 | 8.5 | 11 | 17 |
| 12 | 1.7 | 2.8 | 4.2 | 6.2 | 8.3 | 10 | 14 | 21 |
| 20 | 2.2 | 3.6 | 5.4 | 8.0 | 11 | 13 | 18 | 27 |
| 30 | 2.7 | 4.4 | 6.6 | 9.9 | 13 | 16 | 22 | 33 |
| 50 | 3.5 | 5.7 | 8.5 | 13 | 17 | 21 | 28 | 42 |
| 80 | 4.5 | 7.2 | 11 | 16 | 21 | 27 | 36 | 54 |
| 100 | 5.0 | 8.0 | 12 | 18 | 24 | 30 | 40 | 60 |

## 6. What fills each orbit

With the main world and the giant planets placed, fill every remaining orbit. Roll 2D on the column of Table 21 for the orbit's zone. DM −1 in every column if the primary's mass is below 0.3 (small stars have less material to build with). For an orbit next to a giant planet, treat a result of 5 as Belt: giants shepherd belts, so belts are more common beside them.

**Published counts.** When the numbers of giant planets, belts and worlds in the system are already known (Travellermap lists them for most of Charted Space), do not roll Table 21 for every orbit. Place the published bodies instead, innermost free orbit first, and leave the rest empty:

1. The giant planets are already placed (Section 5.4) and the ice belt, if Table 19 gave one, is already counted as a belt.
2. Each remaining belt takes an orbit: roll 1D, 1 to 4 the innermost free Cold or Outer orbit, 5 or 6 the innermost free orbit of any zone.
3. Each remaining world (the world count less the main world, the belts and the giants) takes the innermost free orbit. Roll 2D on Table 21 for what kind of body it is, reading a result of Empty or Belt as World.
4. If the orbits run out before the bodies do, add orbits as in Section 4.4: outward while there is room inside 100 and clear of any companion's gap, then by splitting the widest gaps, until every body has one.
5. Any orbit still free is Empty. Remove Empty orbits beyond the outermost body; keep the ones inside.

If only the belt and giant counts are known, and not the number of worlds, fill the remaining orbits with Table 21 as usual but read a Belt result as World, since the belts are already accounted for.

**Table 21: What fills an orbit**

| 2D | Inner | Hot | Temperate | Cold | Outer |
| --- | --- | --- | --- | --- | --- |
| 2 | Empty | Empty | Empty | Empty | Empty |
| 3 | Empty | Empty | Belt | Belt | Belt |
| 4 | Belt | Belt | Belt | Belt | Belt |
| 5 | World | World | World | World | World |
| 6 | World | World | World | World | World |
| 7 | World | World | World | World | World |
| 8 | World | World | World | World | World |
| 9 | World | World | World | World | Sub-Neptune |
| 10 | Sub-Neptune | Sub-Neptune | Sub-Neptune | Sub-Neptune | Sub-Neptune |
| 11 | Sub-Neptune | Sub-Neptune | Sub-Neptune | Sub-Neptune | Sub-Neptune |
| 12 | Empty | Sub-Neptune | Sub-Neptune | Sub-Neptune | Icy dwarf |

An Inner-zone orbit inside a hot Jupiter's orbit is Empty without a roll. Table 22 says what each kind of body is and which of them go on to Section 7 for a full set of world codes.

**Table 22: Kinds of body**

| Body | What it is | World codes |
| --- | --- | --- |
| Empty | Nothing of note. Record it; empty orbits are where a referee later hides things | none |
| Belt | A planetoid belt: rock and metal in the Inner and Hot zones, ice further out | Size 0, atmosphere 0, hydrographics 0. A belt in the Cold or Outer zone is an ice source |
| World | A terrestrial planet: anything from an airless rock or a ball of ice to a garden world. Whether it is made of metal, rock or ice is decided by its composition roll in Section 7.2, which the zone weights | Rolled in full in Section 7: size, composition, gravity, atmosphere, hydrographics, temperature |
| Sub-Neptune | The most common kind of planet in the galaxy, between Earth and Neptune in size: a rocky core under a thick hydrogen-helium or steam atmosphere. Not landable and not usefully skimmable | Size: roll 1D, 1 B, 2 C, 3 D, 4 E, 5 to 6 F (17,600 to 24,000 km). Atmosphere: roll 1D, 1 to 4 A (exotic), 5 to 6 B (corrosive). Hydrographics 0. No further world rolls; roll only for its moons (Section 8) |
| Icy dwarf | A small ice body like Pluto | Size 1D3, atmosphere 0, hydrographics 1D + 4 as ice; an ice source. No further world rolls; roll only for its moons (Section 8) |
| Giant planet | Placed in Section 5 | No world codes; kind and size from Table 17. Moons in Section 8 |

## 7. Worlds

Generate each world from Table 21 in this order: size, composition and gravity, atmosphere, hydrographics, temperature. A known main world keeps its published size, atmosphere and hydrographics; only roll its composition and work out its temperature. Population, government, law and tech level are not part of system generation; use the core rules for them.

### 7.1 Size

Roll 2D − 2. DM −2 in the Inner zone, DM −1 if the star's mass is below 0.3. Treat a result below 1 as 1. Size is the diameter in units of 1,600 km, so size 8 is Earth.

### 7.2 Composition and gravity

Roll 1D: **1 or less** iron-rich (a large metal core, like Mercury), **2 to 4** rocky (like Earth or Venus), **5 or more** ice-rock (like Ganymede or Callisto). DM −2 in the Inner and Hot zones, DM −1 in the Temperate zone, DM +1 in the Cold zone, DM +2 in the Outer zone. So a world inside the Hot zone is never ice-rock (any ice it formed with has long since boiled away), a Temperate world occasionally is (an ocean world: built from ice beyond the snow line and carried inward while the system was young, its ice now melted into a deep global sea), and a world past the snow line usually is. An ice-rock world in the Cold or Outer zone is an **ice source**; mark it. Read the surface gravity from Table 23.

**Table 23: Surface gravity by size and composition**

| Size | Diameter (km) | Ice-rock | Rocky | Iron-rich |
| --- | --- | --- | --- | --- |
| 1 | 1,600 | 0.07 | 0.12 | 0.16 |
| 2 | 3,200 | 0.15 | 0.25 | 0.33 |
| 3 | 4,800 | 0.22 | 0.38 | 0.49 |
| 4 | 6,400 | 0.30 | 0.50 | 0.65 |
| 5 | 8,000 | 0.38 | 0.62 | 0.81 |
| 6 | 9,600 | 0.45 | 0.75 | 0.98 |
| 7 | 11,200 | 0.53 | 0.88 | 1.14 |
| 8 | 12,800 | 0.60 | 1.00 | 1.30 |
| 9 | 14,400 | 0.67 | 1.12 | 1.46 |
| A | 16,000 | 0.75 | 1.25 | 1.62 |

Gravity is in Earth gravities. The 100-diameter jump limit of a world is its size × 160,000 km.

### 7.3 Atmosphere

Roll 2D − 7 + size. DM +1 iron-rich, DM −1 ice-rock, DM −2 in the Inner zone, DM −1 in the Outer zone, DM −2 for size 2. A size 1 world has atmosphere 0 without a roll. Results below 0 are 0; results above 15 are 15 (F). Read the code as in the core rules: 0 none, 1 trace, 2–3 very thin, 4–5 thin, 6–7 standard, 8–9 dense, A exotic, B corrosive, C insidious, D dense high, E thin low, F unusual. Where the core rules make a world's atmosphere tainted, do the same here.

### 7.4 Hydrographics

Roll 2D − 7 + size. DM −4 if the atmosphere is 0–1 or A or higher. DM −2 in the Hot zone. Size 1 worlds have hydrographics 0; in the Inner zone every world has hydrographics 0. Results below 0 are 0; above 10 are 10 (A). In the Cold and Outer zones the result is ice, not liquid; write it as the hydrographics digit and note **ice**.

### 7.5 Temperature

Temperature is read, not rolled. Find the world's albedo class in Table 24, take the equilibrium temperature for its position and class from Table 25, add the greenhouse warming for its atmosphere from Table 26, and read the band from Table 27.

The result is a **global average**: the mean over the whole surface and the whole year. No place on the world is at that temperature all the time. On an Earth-like world with a normal day and modest tilt, expect the equator to run 15 to 20 degrees above the average and the poles 30 to 40 below it, so a Temperate world at 15 °C has tropics near 30 and polar caps well below freezing, with a summer and winter on top of that wherever the tilt is above 10° or so. A thicker atmosphere evens things out (Venus is the same temperature everywhere); a thin one exaggerates the spread (Mars swings 100 degrees between day and night). A tidally locked world has no average worth the name: its day side may sit 50 degrees above the figure and its night side 100 below, with a livable ring at the terminator. Use the average to pick the band, and the band to say what the world is like; use the spread to say where on it people live.

**Table 24: Albedo class**

| Class | Column | Which worlds |
| --- | --- | --- |
| Rock | 0.10 | Atmosphere 0–3 and hydrographics 0–3; belts and bare worlds |
| Thin air | 0.20 | Atmosphere 4–9 with hydrographics 0–3; or atmosphere 2–3 with hydrographics 4+ |
| Earth-like | 0.30 | Atmosphere 4 or higher (other than B or C) with hydrographics 4–8 |
| Cloudy | 0.45 | Hydrographics 9–A with atmosphere 4+; atmosphere B or C always |
| Ice | 0.65 | Any world whose hydrographics is ice (Cold or Outer zone with hydrographics 1+), and any world sent here by the ice check below |

**Table 25: Equilibrium temperature in °C by position and albedo class** (before greenhouse warming; use the nearest row)

| Position (HD) | Rock 0.10 | Thin air 0.20 | Earth-like 0.30 | Cloudy 0.45 | Ice 0.65 |
| --- | --- | --- | --- | --- | --- |
| 0.05 | 940 | 905 | 866 | 799 | 685 |
| 0.063 | 808 | 776 | 742 | 682 | 580 |
| 0.08 | 686 | 658 | 627 | 575 | 484 |
| 0.10 | 585 | 560 | 532 | 485 | 404 |
| 0.125 | 494 | 472 | 447 | 405 | 333 |
| 0.16 | 405 | 385 | 364 | 326 | 262 |
| 0.20 | 333 | 316 | 296 | 263 | 206 |
| 0.25 | 269 | 254 | 236 | 207 | 155 |
| 0.32 | 206 | 192 | 177 | 151 | 106 |
| 0.40 | 156 | 143 | 130 | 106 | 66 |
| 0.50 | 110 | 99 | 87 | 66 | 30 |
| 0.63 | 69 | 59 | 48 | 29 | −3 |
| 0.80 | 30 | 21 | 12 | −5 | −34 |
| 0.95 | 5 | −3 | −12 | −27 | −53 |
| 1.0 | −2 | −10 | −18 | −33 | −59 |
| 1.1 | −15 | −22 | −30 | −44 | −69 |
| 1.25 | −31 | −38 | −45 | −59 | −82 |
| 1.4 | −44 | −51 | −58 | −70 | −92 |
| 1.6 | −59 | −65 | −72 | −84 | −104 |
| 1.7 | −65 | −71 | −78 | −89 | −109 |
| 2.0 | −81 | −87 | −93 | −104 | −122 |
| 2.3 | −94 | −99 | −105 | −115 | −132 |
| 2.7 | −108 | −113 | −118 | −127 | −143 |
| 3.2 | −122 | −126 | −131 | −139 | −153 |
| 4.0 | −138 | −141 | −146 | −153 | −166 |
| 5.0 | −152 | −155 | −159 | −166 | −177 |
| 6.3 | −165 | −168 | −172 | −178 | −188 |
| 8.0 | −177 | −180 | −183 | −188 | −197 |
| 10 | −187 | −190 | −193 | −197 | −205 |
| 16 | −205 | −207 | −209 | −213 | −220 |
| 25 | −219 | −220 | −222 | −225 | −230 |
| 40 | −230 | −232 | −233 | −235 | −239 |
| 63 | −239 | −240 | −241 | −243 | −246 |
| 100 | −246 | −247 | −248 | −249 | −252 |

Earth is the 1.0 row, Earth-like column: −18 °C before its atmosphere adds 35.

**Table 26: Greenhouse warming** (add to the equilibrium temperature)

| Atmosphere | 0–1 | 2–3 | 4–5 | 6–7 | 8–9 | A | B | C | D | E | F |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| Add °C | 0 | 5 | 20 | 35 | 60 | 50 | 150 | 250 | 100 | 10 | 35 |

**Table 27: Temperature bands**

| Band | Mean surface temperature |
| --- | --- |
| Frozen | −51 °C and below |
| Cold | −50 to 0 °C |
| Temperate | 1 to 30 °C |
| Hot | 31 to 80 °C |
| Roasting | above 80 °C |

**Ice check.** If the first reading is Cold or Frozen and the world has hydrographics 1 or more, its water freezes and brightens it: move to the Ice column and read again. That is the final answer; do not repeat. If the first reading is Roasting and the world has hydrographics 1 or more, the oceans have boiled: the hydrographics stays as written (it is in the air as steam), the world is Cloudy, and it is Roasting whatever the second reading says.

**Example.** Earth: position 1.0, atmosphere 6, hydrographics 7, so Earth-like: −18 °C from Table 25, plus 35 for a standard atmosphere, gives 17 °C, Temperate. Mars: position 1.5 (nearest row 1.6), atmosphere 1, hydrographics 0, so Rock: −59 °C plus 0, Frozen. A world at 1.4 with atmosphere 8 and hydrographics 7: Earth-like gives −58, plus 60 is 2 °C, Temperate, though only just; with hydrographics 9 it is Cloudy, −70 plus 60 is −10, Cold, and the ice check makes it Frozen at −32. In the outer half of the Temperate zone the star alone is not enough to keep a world above freezing; only a dense atmosphere, trapping the heat, makes a world there livable. That is why Table 10 places a main world with atmosphere 8 or 9 further out than one with atmosphere 6 or 7.

### 7.6 The world record

Write each world as: position, distance (Mkm), travel time (days at thrust 1), zone, size-atmosphere-hydrographics, composition, gravity, temperature band, its moons from Section 8, and (for the full procedure) its rotation, tilt and eccentricity from Section 9. Mark ice sources and giants as in Section 5.

## 8. Satellites and rings

Moons orbit in **planetary radii**: the distance from the planet's centre in units of its own radius. A moon at 3 radii skims the planet; Luna is at 60. Satellite orbits do not depend on the star, except through the stability limit in Section 8.3. Roll moons for every world, sub-Neptune, icy dwarf and giant planet in the system; a world's moons affect its rotation and tilt in Section 9.

### 8.1 Number of moons

**Table 28: Number of moons and rings**

| Body | Moons | Rings |
| --- | --- | --- |
| World, size 3 or more | 1D − 3 (minimum 0) | 2D roll of 12 |
| World, size 1 or 2 | 1D − 4 (minimum 0) | none |
| Sub-Neptune | 1D − 2 (minimum 0) | 1D roll of 6 |
| Ice giant | 1D + 1 | 1D roll of 1 to 3 |
| Gas giant | 2D | 1D roll of 1 to 4 |

A ring is recorded as a satellite with size code **R**, atmosphere 0 and hydrographics 0, and needs no rolls for size or band. It lies inside about 2.5 radii, where the planet's tides tear any moon apart and keep the debris as a ring: roll 1D, 1 to 3 the ring spans 1.5 to 2 radii, 4 to 6 it spans 2 to 3 radii. A ring counts as one of the planet's satellites for Table 28 but not toward the moons rolled there, and a moon rolled in the Close band inside the ring's outer edge is moved just outside it.

### 8.2 Moon size

- Moon of a world or sub-Neptune: **roll 1D. 1 to 3, S: a small moon under 1,600 km (Phobos to Ceres); write S in the size column. 4 or 5, size 1 (or S if the parent is size 3 or less). 6, a large moon: the parent's size ÷ 3, rounded down, minimum 1; for Earth that is size 2, which is Luna**.
- Moon of a giant planet: 2D − 6, minimum S. This gives Ganymede (size 3) at the top and mostly S.
- **Giant planet in the Temperate zone**: its first moon rolls 2D − 3 instead, minimum 1. Such a giant is the natural place for a large habitable moon, and these rules deliberately allow one up to size 9. Generate a moon of size 1 or more as a world (Section 7) at the giant's position, in the giant's zone; its composition roll gets DM +1 (moons are icy).

A **large moon** is one rolled on the 6 above: at least a quarter of its parent's size, as Luna is to Earth. Most worlds have none; about one in six moons is one. A large moon matters in Section 9: it slows its parent's day and steadies its tilt, which is why Earth has a 24-hour day and a steady 23° tilt.

### 8.3 Moon orbits and the stability limit

For each moon roll 1D for its band, then the dice shown for its distance in planetary radii.

**Table 29: Moon orbits** (1D for the band, then the dice shown)

| 1D | Band | Distance (radii) |
| --- | --- | --- |
| 1 to 3 | Close | 2D + 1 (3 to 13) |
| 4 to 5 | Far | (2D − 2) × 5 + 15 (15 to 65) |
| 6 | Extreme | (2D − 2) × 25 + 75 (75 to 325) |

Two moons cannot share a distance; move the second one out by 1D radii.

**Stability limit.** A moon can only stay in orbit while the star's pull on it is weak compared with the planet's, so close to a star the wide bands are not available. Table 5 gives each star a **moon limit**; find the nearest row to it in Table 30 and read across to the planet's position. The cell is the widest orbit, in radii, a moon can have there. A moon rolled beyond it moves in to that distance; "none" means the planet keeps no moons at all. For a giant planet read one row up (giants are big for their mass). Around the habitable zone of a small red dwarf the limit is a handful of radii, which is why those worlds have no large moons and why their moons, when they have any, are close and fast.

**Table 30: Widest moon orbit, in planetary radii.** Rows: the star's moon limit from Table 5. Columns: the planet's position in HD.

| Moon limit ↓ / Position (HD) → | 0.1 | 0.25 | 0.5 | 1 | 2 | 4 | 8 | 16+ |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 4 | none | none | none | 4 | 8 | 16 | 32 | 64 |
| 6 | none | none | 3 | 6 | 12 | 24 | 48 | 96 |
| 8 | none | none | 4 | 8 | 16 | 32 | 64 | 128 |
| 13 | none | 3 | 6 | 13 | 26 | 52 | 104 | 208 |
| 20 | none | 5 | 10 | 20 | 40 | 80 | 160 | 320 |
| 30 | 3 | 7 | 15 | 30 | 60 | 120 | 240 | any |
| 50 | 5 | 12 | 25 | 50 | 100 | 200 | any | any |
| 80 | 8 | 20 | 40 | 80 | 160 | 320 | any | any |
| 120 | 12 | 30 | 60 | 120 | 240 | any | any | any |
| 200 or more | 20 | 50 | 100 | 200 | any | any | any | any |

### 8.4 A moon's day and month

Every moon of size S or larger is locked to its planet: one face toward the planet, and its day equal to its orbital period. Read the period from Table 31 for a size 8, 1 g planet and multiply by √(size ÷ 8 ÷ gravity) for the actual parent; for a gas giant that factor is about 2, for an ice giant about 1.6.

**Table 31: A moon's orbital period** (size 8, 1 g parent; see the multiplier above)

| Radii | Period | Radii | Period |
| --- | --- | --- | --- |
| 3 | 7 h | 45 | 17.7 days |
| 5 | 16 h | 65 | 30.8 days |
| 8 | 32 h | 100 | 59 days |
| 13 | 66 h (2.8 days) | 200 | 166 days |
| 20 | 5.3 days | 325 | 344 days |
| 30 | 9.7 days |  |  |

A large moon of a giant at 8 radii therefore has a day of about 64 hours; at 20 radii, about ten days. The planet hangs fixed in the sky of the near side, going through phases, and eclipses the sun once a day for a moon in a close orbit.

### 8.5 Tidal heating

A moon in a close orbit (band Close) around a giant planet is flexed by the giant's tides and warmed from inside, as Io and Europa are. For its temperature (Section 7.5), read the band one step warmer than the table gives if the table gave Cold or Frozen. Note it as **volcanic** if it is size 3 or more: active volcanoes, a young surface, and a warm ocean under any ice. This is the one place in these rules where a world can be Temperate beyond the habitable zone, and it should be rare: only the Close band of a giant, only in the Cold zone or the inner half of the Outer zone.

## 9. Rotation, tilt and eccentricity

These three facts decide what a day, a season and a year feel like on a world. They belong to the full procedure: roll them for worlds the players will visit. Moons do not use Section 9.2: every sizeable moon is locked to its planet, not its star, and Section 8.4 gives its day.

### 9.1 Eccentricity

Roll 2D on Table 32. Eccentricity is how far the orbit departs from a circle. At 0.1 the world is 10% closer to its star at one end of its year than at the other and its warmth varies by about 40% between them; at 0.3 the near and far ends of the year feel like different zones.

**Table 32: Eccentricity**

| 2D | Eccentricity | Warmth, near end ÷ far end |  | 2D | Eccentricity | Warmth |
| --- | --- | --- | --- | --- | --- | --- |
| 2 | 0.00 | 1.0 |  | 8 | 0.10 | 1.5 |
| 3 | 0.01 | 1.0 |  | 9 | 0.15 | 1.8 |
| 4 | 0.02 | 1.1 |  | 10 | 0.20 | 2.3 |
| 5 | 0.03 | 1.1 |  | 11 | 0.30 | 3.4 |
| 6 | 0.05 | 1.2 |  | 12 | 0.45 | 7.0 |
| 7 | 0.07 | 1.3 |  |  |  |  |

DM −2 for a world whose position is at or below the star's lock limit in Table 5 (tides circularise orbits), DM +1 for a world in a system with a companion star inside 20 HD. A world with eccentricity 0.2 or more should have its temperature band read twice: once at its position × (1 − eccentricity), the near end of its year, and once at its position × (1 + eccentricity), the far end. If the two bands differ, mark the world on the record as **two-season** and write both bands ("Hot / Cold", say). Such a world has a summer and a winter driven by its orbit rather than its tilt, felt everywhere on the world at once, and it deserves two maps, one for each end of the year.

### 9.2 Rotation

First decide whether the world is **tidally locked**: one face always toward its star, a day as long as its year, a hot side and a cold side. Compare the world's position with the star's **lock limit** in Table 5.

- Position **below** the lock limit: the world is locked, unless its eccentricity (Section 9.1) is 0.15 or more, in which case it is in a **3:2 resonance** like Mercury: three rotations for every two orbits, so it has a slow day (its year × 2) and no permanent hot side.
- Position **above** the lock limit: the world rotates freely. Roll 2D on Table 33 for the length of its day.

**Table 33: Day length** (a freely rotating world)

| 2D | Day (hours) |  | 2D | Day (hours) |
| --- | --- | --- | --- | --- |
| 2 | 5 |  | 8 | 24 |
| 3 | 7 |  | 9 | 32 |
| 4 | 9 |  | 10 | 48 |
| 5 | 11 |  | 11 | 72 |
| 6 | 14 |  | 12 | Rare: roll 1D × 100 hours, retrograde on 1D 1 to 2 (like Venus) |
| 7 | 18 |  |  |  |

DM +2 if the world has a large moon (Section 8.2); tides slow a world down.

**The main world is the exception.** A main world is never marked tidally locked by these rules, because its published data and everything written about it in the campaign come first. Work out the physical answer anyway and write it down as "physics: locked" so it is there when you want it. A main world is locked only if the referee decides it is and records the decision. Every other planet follows the rule above as written, unless the referee has recorded "not locked" for it.

### 9.3 Axial tilt

Roll 2D on Table 34. Tilt drives the seasons: near 0 there are none; near 23 they are Earth's; above 45 the poles get more sun than the equator over a year and the climate is strange. A tidally locked world has no meaningful tilt.

**Table 34: Axial tilt**

| 2D | Tilt |  | 2D | Tilt |
| --- | --- | --- | --- | --- |
| 2 | 0° |  | 8 | 25° |
| 3 | 3° |  | 9 | 30° |
| 4 | 6° |  | 10 | 40° |
| 5 | 10° |  | 11 | 55° |
| 6 | 15° |  | 12 | 90° (on its side, like Uranus) |
| 7 | 20° |  |  |  |

DM −2 for a world with a large moon (the moon steadies it) and for a world at or below the lock limit (tides flatten it).

### 9.4 Year

The year is not rolled. For a star of mass M (Table 5) and a world at distance d in Mkm, the year in Earth days is 0.2 × d¹·⁵ ÷ √M. Table 35 covers the usual cases; interpolate between rows.

**Table 35: Length of the year**

| Distance (Mkm) | Year, Sun-like star (mass 1) | Year, red dwarf (mass 0.3) |
| --- | --- | --- |
| 5 | 2.2 days | 4.1 days |
| 10 | 6.3 days | 11.5 days |
| 20 | 18 days | 33 days |
| 40 | 51 days | 92 days |
| 75 | 130 days | 237 days |
| 150 | 367 days | 1.8 years |
| 300 | 2.8 years | 5.2 years |
| 800 | 12 years | 23 years |
| 1,500 | 32 years | 58 years |
| 4,500 | 165 years | 301 years |

For a red dwarf's habitable zone the year is days, not months: a garden world around an M5 V at 9 Mkm has a year of about ten days, and if it is tidally locked, that is also its day.

## 10. Settlement of the other worlds

The main world is where the system's people live, but a system with a main world of any size usually has outposts, mines, farms and stations on its other worlds, moons and belts. This section gives each of them a population, a government, a law level, a tech level and a spaceport, in that order, and says why anyone is there. It is part of the quick procedure. Roll it for every world, moon of size 1 or more, and belt in a system that has a main world with population 1 or more; in a system with no main world, or a main world with population 0, every other body has population 0 unless the referee says otherwise.

The rolls lean on what the earlier sections found: a warm world with breathable air and water draws settlers, and a frozen rock a month from anywhere does not. That is the point of having generated the system before its people.

### 10.1 Population

Roll 2D − 2 with the DMs in Table 36. A result below 0 is 0. The result is never higher than the main world's population minus 1; reduce it if it is. Population is the usual exponent: 3 means thousands, 5 hundreds of thousands. Two kinds of settlement are covered by one roll. On a world with a surface worth living on, the surface rows apply: warmth, air and water decide how many come. A belt has no surface, and its people live in habitats and hollowed rocks where the outside does not matter; what matters is whether the main world's technology can support life off-world at all, so a belt skips the surface rows and uses the belt row instead.

**Table 36: Population DMs for other worlds**

| Condition | DM |
| --- | --- |
| **Surface rows** (worlds and moons; a belt skips these) |  |
| Temperature band Temperate | 0 |
| Temperature band Hot or Cold | −2 |
| Temperature band Roasting or Frozen | −4 |
| Atmosphere 5, 6 or 8 (breathable, untainted) | 0 |
| Atmosphere 4, 7 or 9 (breathable, tainted) | −1 |
| Atmosphere 2, 3, A, D, E or F | −2 |
| Atmosphere 0 or 1 (vacuum or trace) | −3 |
| Atmosphere B or C (corrosive or insidious) | −4 |
| Hydrographics 0 | −1 |
| Size 1 or 2, or a moon of size 1 or 2 | −1 |
| Gravity above 1.5 | −1 |
| A moon in the Close band of a gas giant (radiation belts) | −2 |
| **Belt row** (instead of the surface rows) |  |
| Belt | −4 |
| **Rows for every body** |  |
| The system's nearest fuel source, or a moon of it | +1 |
| Main world tech level 12 or more | +2 |
| Main world tech level 7 or 8 | +1 |
| Main world tech level 7 or less | −2 |
| Main world industrial (In) or rich (Ri) | +1 |
| Main world population 8 or more | +1 |
| Main world population 3 or less | −2 |
| Main world population 0, or main world tech level 6 or less (no spaceflight) | population 0, no roll |

So a belt in a system whose main world is tech level 9 or better and reasonably populated is settled more often than not, by a community in the hundreds to hundreds of thousands: belters are ordinary in Charted Space, and the rules should produce them. At tech level 7 or 8 they are rare, and at tech level 6 or less no one has left the main world at all. A moon of a giant planet with population is usually a fuel depot or a mining camp; an airless rock near the star is an outpost of a few dozen; a Temperate world with air and water and a good roll is a second inhabited world, and the referee should give it a name.

### 10.2 Government

A world with population 0 has government 0. For population 1 or 2, roll 1D: 1 to 3, government 0 (a few families, no formal rule); 4 to 6, government 1 (a company outpost). For population 3 or more, roll 1D on Table 37.

**Table 37: Government of other worlds** (DM +1 if the main world's government is 7 or higher; if the main world's government is 6, the result is 6 without a roll)

| 1D | Code | Government |
| --- | --- | --- |
| 1 | 0 | None; family or clan bonds |
| 2 | 1 | Company or corporation |
| 3 | 2 | Participating democracy |
| 4 | 3 | Self-perpetuating oligarchy |
| 5 or more | 6 | Captive government: administered from the main world |

### 10.3 Law level

Roll 1D − 3 and add the main world's law level. A world with government 0 has law level 0. A result below 0 is 0.

### 10.4 Facilities

Check each line of Table 38 in order; a world may qualify for more than one. Facilities are the reason a settlement exists, and they feed the tech level and spaceport below.

**Table 38: Facilities**

| Facility | Requirements |
| --- | --- |
| Farming | Temperature band Temperate, atmosphere 4 to 9, hydrographics 4 to 8, population 2 or more |
| Mining | Population 2 or more, and either the world is a belt, or its composition is iron-rich, or the main world is industrial (trade code In) |
| Fuel depot | The world is the system's nearest fuel source (a giant planet's moon, or a charted ice body) and has population 1 or more |
| Colony | Government 6 and population 5 or more |
| Research station | Roll 2D for 11 or more. DM +2 if the main world's tech level is 10 or more; DM +1 if the world is remarkable (two-season, volcanic, tidally locked, a corrosive or insidious atmosphere, or an ocean world). Not possible if the main world's tech level is 8 or less, or the world's population is 0 |
| Military base | Roll 2D for 12 or more. DM +1 if the main world's population is 8 or more; DM +2 if the world's atmosphere equals the main world's. Not possible if the world's population is 0 or the main world is poor (trade code Po) |

### 10.5 Tech level

The world's tech level is the main world's minus 1. It equals the main world's if the world has a research station or a military base. If the world has population 1 or more and needs life support to live on, which means its atmosphere is not 5, 6 or 8 or its temperature band is not Temperate, its tech level is at least 7; raise it to 7 if it would be lower. A world with population 0 has tech level 0.

### 10.6 Spaceport

Roll 1D on Table 39. Spaceports are the small ports of a system's other worlds; the main world's port is the starport.

**Table 39: Spaceport** (DM +2 if population 6 or more; DM −2 if population 1; DM −3 if population 0; DM +1 for a fuel depot or a colony)

| 1D | Code | Spaceport |
| --- | --- | --- |
| 2 or less | Y | None |
| 3 | H | Primitive: a landing field and nothing else |
| 4 or 5 | G | Poor: unrefined fuel, no repairs |
| 6 or more | F | Good: unrefined fuel, minor repairs |

Write the result on the record as an extended profile beside the world codes, spaceport first: F-4-1-3-A, say, for a good spaceport, population in the tens of thousands, company rule, law level 3, tech level 10.

## 11. Travel and refuelling reference

### 11.1 Travel time

Time to cross a distance under constant thrust, accelerating to the midpoint and decelerating from it. The distance between two bodies in the same system varies with where they are in their orbits, from the difference of their distances from the star to the sum; for planning, use the larger of the two distances.

**Table 40: Travel time by distance and thrust**

| Distance (Mkm) | Thrust 1 | Thrust 2 | Thrust 4 | Thrust 6 |
| --- | --- | --- | --- | --- |
| 1 | 6 h | 4 h | 3 h | 2 h |
| 2 | 8 h | 6 h | 4 h | 3 h |
| 5 | 13 h | 9 h | 6 h | 5 h |
| 10 | 18 h | 13 h | 9 h | 7 h |
| 20 | 1.0 d | 18 h | 13 h | 10 h |
| 50 | 1.7 d | 1.2 d | 20 h | 16 h |
| 100 | 2.3 d | 1.7 d | 1.2 d | 23 h |
| 150 | 2.9 d | 2.0 d | 1.4 d | 1.2 d |
| 300 | 4.1 d | 2.9 d | 2.0 d | 1.7 d |
| 500 | 5.2 d | 3.7 d | 2.6 d | 2.1 d |
| 800 | 6.6 d | 4.7 d | 3.3 d | 2.7 d |
| 1,500 | 9.1 d | 6.4 d | 4.5 d | 3.7 d |
| 3,000 | 12.8 d | 9.1 d | 6.4 d | 5.2 d |
| 5,000 | 16.5 d | 11.7 d | 8.3 d | 6.8 d |
| 9,000 | 22 d | 15.7 d | 11.1 d | 9.1 d |
| 30,000 | 41 d | 29 d | 20 d | 16.5 d |

Formula: days at thrust 1 = 0.234 × √(Mkm); divide by √(thrust) for higher ratings. Doubling the distance adds only 40% to the time, which is why the whole planetary region of a Sun-like star is within a few weeks and why a companion star at 30,000 Mkm is not.

### 11.2 Jump shadows

A ship cannot jump from inside the 100-diameter limit of any body and normally arrives at the edge of the limit around its destination. Table 5 gives each star's shadow. For worlds it is size × 160,000 km: 1.3 Mkm for a size 8 world, about six hours out at thrust 1. For giant planets: ice giant 5 Mkm, Saturn-class 9 Mkm, Jupiter-class 14 Mkm. A ship jumping to a giant to refuel arrives at that edge and takes 13 to 18 hours to reach the cloud tops.

Where a world's orbit lies inside its star's shadow (every habitable world of an M dwarf, and anything inside 139 Mkm of a Sun-like star), a ship cannot jump directly to the world: it arrives at the star's shadow and flies in. For an M6 V that is 18 Mkm from the star, a day's flight at thrust 1 to a world at 5 Mkm.

### 11.3 Refuelling rates

From the Drinaxian Companion. Each full 25 tons of fuel tankage is one Fuel Transfer Unit (FTU), minimum 1. Rates are per hour, per FTU, so a full tank takes about the same time on any ship.

**Table 41: Refuelling rates**

| Source | Rate per FTU per hour | Full tank | Notes |
| --- | --- | --- | --- |
| Starport A–C | 50 tons | about 30 minutes | Refined or unrefined as sold |
| Starport D | 25 tons | about an hour | Usually unrefined |
| Gas or ice giant, skimming | 2D + Effect of an Average (8+) Pilot check, Thrust as DM | 3 to 4 hours | Difficult (10+) Pilot check in the Core Rulebook; unrefined; streamlined or partially streamlined hull only |
| Liquid water, by hose | 3D3 tons | about 4 hours | Landed, unrefined |
| Ice deposits | 1D tons | about 7 hours | Landed, unrefined; hoses moved between deposits |
| Sparse ice | D3 tons | about 12 hours | Comets, thin ice on a rock world |
| Brown dwarf, skimming | as a gas giant |  | 120 g at the cloud tops and a fierce wind; treat as a Very Difficult (12+) Pilot check |

Fuel gathered in the wild is unrefined: DM −2 on the jump check and a misjump risk until it is refined. A ship's fuel processors clean 20 tons per ton of processor per day, so the refining, not the gathering, usually sets the pace: a 200-ton trader with one ton of processor spends about two days cleaning a 40-ton tank whatever the source. For the purposes of these rules an ice source is treated as equivalent to a giant: slower by a factor of two, and needing a landing, but a refuelling stop.

**Hazards by source.** A belt is not a hazard to fly through: the bodies in one are millions of kilometres apart and drift past each other at walking pace by spacecraft standards, so the picture of dodging tumbling rocks is wrong. The difficulties of belt refuelling are different. A small body has almost no gravity, so the ship must anchor and the hoses must be moved from deposit to deposit (the sparse-ice rate); a comet near the inner system vents gas and dust from its sunward side; and the surface is often loose rubble that a landing leg can sink into. An icy world or a large icy moon is the easy case: real gravity, a solid surface, a landing rather than an anchoring. The two sources with real danger are the ones a ship has to fly into. Skimming a giant planet is a dive through turbulence at high speed, which is why it takes a Pilot check, and the inner moons of a gas giant sit inside its radiation belts: the space around Jupiter's Io would kill an unshielded crew in a day, so refuelling at a giant's close moons is done quickly or from the far side.

Finding an ice body when none is charted is a separate problem; the Deepnight Revelation Referee's Handbook gives 1D hours in a dense system to 12D hours in a very sparse one with an Electronics (sensors) or Science (cosmology) check. A system generated with these rules always charts its ice sources, so that search is only needed for a system the players know nothing about.

## 12. Worked examples

Three systems generated with these rules, with every roll shown. The first two were rolled freely; the third is Noricum in the Trojan Reach, built around its published data. Positions are rounded to two figures as the rules say.

### 12.1 A free system with a habitable main world

The core rules gave a main world of size 7, atmosphere 6, hydrographics 7, so the habitable column applies.

**Stars.** Number of stars 2D = 5: one star. Spectral class 2D = 10 on the habitable column of Table 3: F. Subtype 2D = 2, minus 2: 0. Class V without a roll. Table 5, F0 V: 1 HD is 347 Mkm, days to 1 HD 4.4, jump shadow 209 Mkm, innermost orbit 0.05, lock limit 0.20, moon limit 231.

**Main world position.** Atmosphere 6, hydrographics 7: band 0.90 to 1.15. 1D = 3: two fifths of the way in, position 1.0. Distance 347 Mkm, 4.4 days.

**Orbits.** Number of orbits (Section 4.2, 2D − 2): 2D = 6, minus 2, gives 4. Split (Section 4.3): 1D = 5, two thirds of the other three inward, so 2 inward and 1 outward. Inward ratios 2D = 6 (1.65) and 2D = 3 (1.35): 1.0 ÷ 1.65 = 0.61, then 0.61 ÷ 1.35 = 0.45. Outward ratio 2D = 5 (1.55): 1.0 × 1.55 = 1.6. Zones: 0.45 Inner, 0.61 Hot, 1.0 and 1.6 Temperate.

**Giants.** Presence 2D = 5: yes. Number 2D = 3: one. No Cold or Outer orbit exists, so add one at 2.7 or 1.6 × 1.75 = 2.8, whichever is further: 2.8, Outer, 971 Mkm, 7.3 days. Kind 2D = 9: gas giant; 1D = 1: Saturn-class. Placement 1D = 5, Cold: none free, so the first free Cold or Outer orbit, 2.8. Ice belt 1D = 1: a belt beyond the last giant, but there is no orbit beyond it, so there is none; the giant's own moons carry the system's ice.

**Refuelling line.** Transit fuel: yes, gas giant. Local fuel: main world 347 Mkm, giant 971 Mkm; use 971, which is 7.3 days at thrust 1. Reachable on a normal fuel load, a week each way.

**Filling the orbits.** Orbit 0.45, Inner column, 2D = 8: rocky world. Size 2D = 11, minus 2, minus 2 for Inner: 7. Composition 1D = 1, minus 1: iron-rich, gravity 1.14. Atmosphere 2D = 10, minus 7, plus 7, plus 1 iron-rich, minus 2 Inner: 9, dense. Hydrographics 0 in the Inner zone. Temperature: thin-air class (atmosphere 4–9, hydrographics 0), row 0.40 gives 143 °C, plus 60 for a dense atmosphere: about 200 °C, Roasting. A Venus. Orbit 0.61, Hot column, 2D = 5: rocky world. Size 2D = 3, minus 2: 1. Size 1, so atmosphere 0 and hydrographics 0; composition 1D = 5, minus 1: rocky, gravity 0.12. Temperature: rock class, row 0.63 gives 69 °C, Hot. A large airless rock. Orbit 1.6, Temperate column, 2D = 7: rocky world. Size 2D = 6, minus 2: 4. Composition 1D = 3, minus 1: rocky, gravity 0.50. Atmosphere 2D = 11, minus 7, plus 4: 8, dense. Hydrographics 2D = 3, minus 7, plus 4: 0. Temperature: thin-air class, row 1.6 gives −65, plus 60: −5 °C, Cold. A cold desert under a heavy sky.

**The main world in full.** Composition 1D = 4, minus 1: rocky, gravity 0.88. Temperature: Earth-like class, row 1.0 gives −18, plus 35: 17 °C, Temperate. Moons 1D = 3, minus 3: none. Position 1.0 is above the lock limit 0.20, so it rotates freely: day 2D = 6: 14 hours. Tilt 2D = 4: 6°, mild seasons. Eccentricity 2D = 3: 0.01. Year: 0.2 × 347^1.5 ÷ √1.61 = 1,020 days, about 2.8 standard years. Its sun is a white F0 star, brighter and hotter than Sol, and the sky is a deeper blue.

**The giant's moons.** 2D = 7 moons; rings 1D = 6: none. Sizes 2D − 6: 11 gives 5, 4 gives S, 5 gives S, 4 gives S, 9 gives 3, 5 gives S, 7 gives 1. Moon limit 231 × 2.8 × 0.6 = 388 radii, so every band is available. The size 5 moon is a world in its own right, 8,000 km across, and it is in the Outer zone, so it is an ice world and a fuel source of the slower kind.

| Position | Mkm | Days | Zone | Body | UWP digits | Notes |
| --- | --- | --- | --- | --- | --- | --- |
| 0.45 | 156 | 2.9 | Inner | Rocky world | 7-9-0 | Iron-rich, 1.14 g, Roasting |
| 0.61 | 212 | 3.4 | Hot | Rocky world | 1-0-0 | 0.12 g, Hot |
| 1.0 | 347 | 4.4 | Temperate | Main world | 7-6-7 | 0.88 g, 17 °C, day 14 h, year 2.8 yr, no moon |
| 1.6 | 555 | 5.5 | Temperate | Rocky world | 4-8-0 | 0.50 g, Cold |
| 2.8 | 971 | 7.3 | Outer | Gas giant, Saturn-class |  | 7 moons incl. a size 5 ice world; fuel |

### 12.2 A red dwarf with no main world

A scout survey of an unnamed M4 V. We take the star as given and roll the rest with no main world. Table 5, M4 V: 1 HD is 13 Mkm, days to 1 HD 0.8, jump shadow 36 Mkm, innermost orbit 0.14, lock limit 2.88, moon limit 16. Everything in this system is close: its position 1.0 is 13 Mkm out, a tenth of Mercury's distance from the Sun.

**Orbits.** Number of orbits (Section 4.2, 2D − 2): 2D = 5, minus 2, gives 3. First orbit (Table 11): 2D = 6, 0.125, which is 1.6 Mkm; the shadow ÷ 20 is 1.8 Mkm, so the orbit moves out to 1.8 Mkm, position 0.14. Ratios 2D = 8 (1.90) and 2D = 2 (1.25): 0.27 and 0.34. All three are Inner: 1.8, 3.4 and 4.3 Mkm, each under half a day from the star.

**Giants.** Presence 2D = 4: yes. Number 2D = 7: three. No Cold or Outer orbit, so add one at 2.7 (34 Mkm, 1.4 days) and drop the two giants that will not fit. Kind 2D = 10: gas giant; 1D = 1: Saturn-class. Placement 1D = 4: Outer, the new orbit. Ice belt 1D = 4: beyond the last giant, but there is nothing beyond it, so none.

**Worlds.** All three Inner orbits get DM −1 for a star under 0.3 solar masses. Orbit 0.14: 2D = 8, minus 1: rocky. Size 2D = 9, minus 2, minus 2 Inner, minus 1 small star: 4. Composition 1D = 2, minus 1: iron-rich, 0.65 g. Atmosphere 2D = 5, minus 7, plus 4, plus 1, minus 2: 1, trace. Rock class at row 0.125: 494 °C, Roasting. Orbit 0.27: 2D = 6, minus 1: rocky. Size 2D = 7: 2. Composition 1D = 3, minus 1: rocky, 0.25 g. Atmosphere 2D = 9, minus 7, plus 2, minus 2 Inner, minus 2 size 2: 0. Row 0.25: 269 °C, Roasting. Orbit 0.34: 2D = 8, minus 1: rocky. Size 2D = 7: 2. Composition 1D = 6, minus 1: ice-rock, 0.15 g. Atmosphere 0. Row 0.32: 206 °C, Roasting. All three are below the lock limit, so each keeps one scorched face to the star; with eccentricity rolls of 2D = 3 and lower they are locked rather than in resonance.

**What a ship sees.** The star's jump shadow is 36 Mkm and the giant orbits at 34 Mkm, inside it. A ship jumping in arrives at the shadow's edge and is eight hours from the cloud tops at thrust 1. Transit fuel: yes. There is no main world, so no local-fuel line. The three inner worlds are rocks at a few hundred degrees a few hours' flight from the giant: a refuelling stop and nothing more, unless the referee puts something in the giant's moons (2D for their number; a habitable one is not possible this far out, but a mining outpost is).

### 12.3 Noricum (Trojan Reach 2018), from published data

Travellermap gives Noricum as D8867BB-1, stars **G2 V M9 V M6 V**, 4 gas giants, no planetoid belts, and 14 bodies in all counting the main world. Everything published is kept. The main world is size 8, atmosphere 8 (dense), hydrographics 6. This is a crowded system, and it shows how the published count is honoured when the orbits run short.

**Stars.** G2 V primary as listed: Table 5 gives 1 HD as 150 Mkm, days to 1 HD 2.9, shadow 139 Mkm, lock limit 0.40, moon limit 117. Companions M9 V and M6 V, as listed, so only separations are rolled on Table 8. M9: 2D = 11: 2,000 HD, which is 300,000 Mkm, 130 days; worlds orbit the primary alone out to 670 HD, so it never touches the orbits. M6: 2D = 6: 6 HD, which is 900 Mkm, 7 days; worlds orbit the primary alone out to 2 HD and both stars beyond 18 HD. Table 9: the M9's separation is more than three times the M6's, so both orbit the primary independently. Position 1.0 is inside the 2 HD limit: the habitable zone is stable.

**Main world position.** Atmosphere 8: band 1.10 to 1.40. 1D = 6: 1.4, which is 209 Mkm, 3.4 days.

**Orbits.** Number of orbits (Section 4.2, 2D − 2): 2D = 4, minus 2, gives 2, but the system is known to have 14 bodies, so 14 orbits. Split 1D = 1: one third of the other 13 inward: 4 inward, 9 outward. Inward ratios 2D = 8 (1.90), 9 (2.05), 7 (1.75) and 10 (2.25): 0.74, 0.36, 0.21 and 0.091. Outward ratios 2D = 4 (1.45), 10 (2.25), 7 (1.75) and 10 (2.25): 2.0, 4.5, 7.9 and 18. The M6 companion's gap runs from 2 HD to 18 HD, which crosses out 4.5, 7.9 and 18 (673, 1,182 and 2,693 Mkm). Keep multiplying: 2D = 8 (1.90): 34, at 5,086 Mkm; 2D = 10 (2.25): 76, at 11,370 Mkm; 2D = 6 (1.65): 130, capped at 100, 14,960 Mkm, and there the outward run stops. That is 9 stable orbits for 14 bodies, so the other 5 come from splitting the widest gaps (Section 4.4), with the edges of the companion gap counting as neighbours. Widest first: 0.091 to 0.21 (ratio 2.3) gives 0.14; 34 to 76 (2.2) gives 51; 0.36 to 0.74 (2.1) gives 0.52; 0.74 to 1.4 (1.9) gives 1.0; the gap's outer edge at 18 to 34 (1.9) gives 25. Fourteen orbits: 0.091, 0.14, 0.21, 0.36, 0.52, 0.74, 1.0, 1.4, 2.0, 25, 34, 51, 76 and 100. The outer five circle the primary and the M6 together.

**Giants.** Four, as published. Kinds (Table 17): 2D = 4, ice giant; 2D = 3, ice giant; 2D = 9, gas giant, 1D = 2, Saturn-class; 2D = 10, gas giant, 1D = 6, Jupiter-class. Placement 1D = 1: the first free Outer orbit, 25. The others take the next free Cold or Outer orbits outward: 34, 51 and 76. Ice (Table 19) 2D = 7: charted, but the published count has no belts, so the ice is in the moons of the Jupiter-class giant at 76.

**Refuelling line.** Transit fuel: yes, giant planet. Local fuel: the nearest ice source turns out to be the frozen world at 2.0 (below), 299 Mkm and 4.0 days at thrust 1 from the main world, which is convenient. The nearest giant is at 25, 3,740 Mkm and 14 days: reachable on a normal fuel load but not convenient. The M6 companion at 900 Mkm may have giants of its own inside its 300 Mkm limit, a week away; roll its system if it matters.

**Filling the orbits.** The giants and the main world are placed and there are no belts, so the nine other published worlds take the nine free orbits, innermost first (Section 6, Published counts), and no orbit is left empty. Each rolls 2D on Table 21 for its kind, reading Empty or Belt as World. Orbit 0.091, Inner, 2D = 7: world. Size 2D = 9, minus 2, minus 2 Inner: 5. Composition 1D = 5, minus 2: rocky, 0.62 g. Atmosphere 2D = 6, minus 7, plus 5, minus 2: 2. Hydrographics 0 in the Inner zone. Rock class at row 0.10: 585 °C, plus 5: 590 °C, Roasting; below the lock limit, so locked. Orbit 0.14, 2D = 3: Empty, read as world. Size 2D = 5, minus 4: 1, so atmosphere 0 and hydrographics 0. Composition 1D = 2, minus 2: iron-rich, 0.16 g. Row 0.125: 494 °C, Roasting, locked. Orbit 0.21, 2D = 10: sub-Neptune. Size 1D = 3: D; atmosphere 1D = 2: A. A hot sub-Neptune, not landable. Orbit 0.36, 2D = 6: world. Size 2D = 11, minus 4: 7. Composition 1D = 6, minus 2: rocky, 0.88 g. Atmosphere 2D = 9, minus 7, plus 7, minus 2: 7. Hydrographics 0. Thin-air class at row 0.40: 143 °C, plus 35: 178 °C, Roasting, locked. A big Venus with no water. Orbit 0.52, Hot, 2D = 8: world. Size 2D = 7, minus 2: 5. Composition 1D = 3, minus 2: iron-rich, 0.81 g. Atmosphere 2D = 8, minus 7, plus 5, plus 1: 7. Hydrographics 2D = 10, minus 7, plus 5, minus 2 Hot: 6. Earth-like class at row 0.50: 87 °C, plus 35: 122 °C, Roasting with water, so cloudy class instead: 66, plus 35: 101 °C, still Roasting. A steam world under permanent cloud. Orbit 0.74, Hot, 2D = 5: world. Size 2D = 8, minus 2: 6. Composition 1D = 4, minus 2: rocky, 0.75 g. Atmosphere 2D = 4, minus 7, plus 6: 3. Hydrographics 2D = 6, minus 7, plus 6, minus 2: 3. Rock class at row 0.80: 30 °C, plus 5: 35 °C, Hot. A hot Mars with lakes. Orbit 1.0, Temperate, 2D = 7: world. Size 2D = 6, minus 2: 4. Composition 1D = 4, minus 1: rocky, 0.50 g. Atmosphere 2D = 7, minus 7, plus 4: 4. Hydrographics 2D = 8, minus 7, plus 4: 5. Earth-like class at row 1.0: −18 °C, plus 20: 2 °C, Temperate. A small, cool, wet world with thin air: Noricum's neighbour, and a second habitable world in the system. Orbit 2.0, Cold, 2D = 9: world. Size 2D = 8, minus 2: 6. Composition 1D = 3, plus 1: rocky, 0.75 g. Atmosphere 2D = 5, minus 7, plus 6: 4. Hydrographics 2D = 9, minus 7, plus 6: 8, ice. Ice class at row 2.0: −122 °C, plus 20: −102 °C, Frozen. A snowball, and an ice source four days from the main world. Orbit 100, Outer, 2D = 12: icy dwarf. Size 1D3 = 2; hydrographics 1D = 3, plus 4: 7, ice. Row 100: −252 °C, Frozen. A Pluto, 29 days out.

**The main world in full.** Composition 1D = 3, minus 1: rocky, 1.0 g. Temperature: Earth-like class at row 1.4: −58, plus 60 for a dense atmosphere: 2 °C, Temperate but only just; a cool world with a thick sky, glaciated at the poles. Moons 1D = 4, minus 3: one. Size 1D = 6: a large moon, size 8 ÷ 3 = 2. Band 1D = 2: Close; distance 2D = 3, plus 1: 4 radii; the limit is 117 × 1.4 = 164, so it stays. A 3,200 km moon at 25,000 km, its month 11 hours, fourteen times the width of Luna in Earth's sky. Rotation: above the lock limit, free; 2D = 4, plus 2 for the large moon: 6, a 14-hour day. Tilt 2D = 6, minus 2 for the large moon: 4, 6°. Eccentricity 2D = 8: 0.10, so the warmth varies by half between the ends of its 1.65-year year (0.2 × 209^1.5 = 604 days): a real winter and summer even with little tilt. Physics says: not locked.

| Position | Mkm | Days | Zone | Body | UWP digits | Notes |
| --- | --- | --- | --- | --- | --- | --- |
| 0.091 | 14 | 0.9 | Inner | Rocky world | 5-2-0 | 0.62 g, 590 °C, Roasting, locked |
| 0.14 | 21 | 1.1 | Inner | Rocky world | 1-0-0 | Iron-rich, 0.16 g, 494 °C, Roasting, locked |
| 0.21 | 31 | 1.3 | Inner | Sub-Neptune | D-A-0 | Not landable |
| 0.36 | 54 | 1.7 | Inner | Rocky world | 7-7-0 | 0.88 g, 178 °C, Roasting, locked |
| 0.52 | 78 | 2.1 | Hot | Rocky world | 5-7-6 | Iron-rich, 0.81 g, 101 °C, Roasting; a steam world |
| 0.74 | 111 | 2.5 | Hot | Rocky world | 6-3-3 | 0.75 g, 35 °C, Hot; a hot Mars with lakes |
| 1.0 | 150 | 2.9 | Temperate | Rocky world | 4-4-5 | 0.50 g, 2 °C, Temperate; small, wet, thin air |
| 1.4 | 209 | 3.4 | Temperate | Noricum (main world) | 8-8-6 | 1.0 g, 2 °C, day 14 h, year 1.65 yr, large moon at 4 radii |
| 2.0 | 299 | 4.0 | Cold | Rocky world | 6-4-8 | 0.75 g, −102 °C, Frozen; ice, the nearest fuel |
| 4.5 to 18 | 673 to 2,693 |  |  | Unstable |  | Cleared by the M6 companion at 900 Mkm |
| 25 | 3,740 | 14 | Outer | Ice giant |  | Circles both stars; nearest giant |
| 34 | 5,086 | 17 | Outer | Ice giant |  |  |
| 51 | 7,630 | 20 | Outer | Gas giant, Saturn-class |  | Split orbit |
| 76 | 11,370 | 25 | Outer | Gas giant, Jupiter-class |  | Charted ice in its moons |
| 100 | 14,960 | 29 | Outer | Icy dwarf | 2-0-7 | Ice; a Pluto |
| Companion | 900 | 7.0 |  | M6 V |  | Own orbits out to 300 Mkm, if rolled |
| Companion | 300,000 | 130 |  | M9 V |  |  |

Nothing published was changed. Fourteen bodies fit inside 100 HD once the five widest gaps were split, and none had to go beyond the far companion. The two Temperate-zone worlds a third of an HD apart are the kind of neighbours real systems have, and the one at 1.0 is worth a note on the record: a second habitable world in a system whose published data mentions only Noricum itself.

## 13. Options and design notes

### 13.1 Optional rules

**Realistic giant frequency.** Table 15 gives giant planets to 83% of systems, Traveller's traditional rate. Surveys of nearby stars find nearer 60% once ice giants are counted. A referee who prefers that rolls 9 or less on Table 15 as 7 or less. Nothing else changes; the ice rules in Section 5.5 already carry the systems that miss.

### 13.2 When published facts do not fit

When a system is generated from published data (Travellermap, the Traveller Wiki, a sourcebook), the following are fixed and are never changed: the main world's UWP; the stars as listed; the number of gas giants and belts; the count of worlds; and anything the referee has already recorded for the system. Everything else is free: orbit positions, the compositions, atmospheres and water of the other worlds, moons, rotation, tilt, eccentricity.

Sometimes the fixed facts cannot be made physically consistent with each other. A garden world whose only listed star is a white dwarf is the clearest case. Do this: honour the facts; generate the system anyway, placing the main world by Table 10 as usual; and write on the record what the physics would have said ("physics: no habitable orbit around this star"), so that the oddity is visible rather than buried. Two kinds of oddity have easy stories. A small world with a thick atmosphere is cold and dense with heavy gases; an airless world with hydrographics has ice, not seas. Two kinds do not: a habitable world around a white dwarf or a giant star, and a temperature code that contradicts the world's atmosphere and water. For the first, look for a companion star to host the world (Section 3.5); for the second, ignore the temperature code, as Section 4.1 says.

When the star is not listed (a bare UWP, or an entry with no stars given), Table 3 chooses one that suits the main world, and Table 10 chooses the orbit that suits it. That is the ordinary case, and it never needs a note.

### 13.3 Where the numbers come from

Table 5 is the Mamajek mean dwarf sequence, interpolated by subtype. One HD is 149.6 Mkm × √(light), the distance at which a world receives Earth's sunlight; the Temperate zone of 0.95 to 1.7 HD is the Kopparapu habitable zone, with the outer half needing a dense atmosphere, which Table 26 supplies. Equilibrium temperatures are 278.5 K × (1 − albedo)^¼ ÷ √(position). The lock limit is 0.4 AU × (mass)^⅓ ÷ √(light), a standard tidal-locking estimate for a world four and a half billion years old. The moon limit is half the Hill radius for an Earth-density planet, which in planetary radii depends only on the distance from the star, the star's mass and the planet's density. Orbit spacing ratios of 1.25 to 2.8 cover the Solar System and the Kepler multi-planet systems. Companion separations follow the Duquennoy and Mayor distribution, which peaks near 30 to 50 AU for Sun-like stars and closer for red dwarfs. Refuelling rates are from the Drinaxian Companion.

### 13.4 What is deliberately not physical

The giant-planet rate (83% rather than about 60%). The frequency of large moons around giant planets in the habitable zone, and their permitted size. The absence of any rule stripping atmospheres from red-dwarf worlds by flares and stellar wind, which is a live debate and which would cost the setting most of its habitable worlds. In each case the setting comes first, and the rule says so where it applies.
