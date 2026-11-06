# Carriages

2026-11-06, installed vanilla assets, examined externally. Actor.pex declares
SetVehicle(ObjectReference); ObjectReference.pex declares
TetherToHorse(ObjectReference). CartAndHorseTether calls TetherToHorse on load
with TheHorse, after positioning/restraining it in its stopped branch.
CartDriverScript attaches the driver with SetVehicle(myCart), then plays its
separate driver idle; detaching uses SetVehicle(None) and a separate exit idle.
Thus SetVehicle must preserve the passenger's placed offset and must not choose
an idle or a seat index itself. MQ101CartRiderScript listens for ExitCartEnd;
it then updates Variable01 and evaluates the package.

PrisonerCarriage01.nif is dynamic (root mass 12000), with wheel hinge constraints
and a limited steering hinge (about ±0.698 radians) for FrontBar. The ordinary
loose-object loader already instantiates these constraints. CartFurnStatic01.nif
is static: it must not be converted into a dynamic carriage merely by attachment.
The external `nif-dump` and `pex-dump` outputs are examination aids only; no asset
or bytecode has been copied into the repository.
