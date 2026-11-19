; Outdoors, where the run began: what makes GetDisease true. Skipped when
; the run began indoors.

; An actor of base b placed x / y from the start marker.
Actor Function SpawnOut(ActorBase b, Float x, Float y)
    Actor a = StartMarker.PlaceActorAtMe(b, 4, None)
    a.MoveTo(StartMarker, x, y, 0.0, false)
    return a
EndFunction

; A creature of base b, whose attacks carry disease d (its race's attack
; data), attacks the player, who doesn't fight back, for up to 90 s or
; until the player has d. Its state and distance are read at 2 s; the
; player's health is topped up, and each drop counts as a hit.
String Function Carrier(ActorBase b, Spell d, String label)
    Player.RemoveSpell(d)
    Actor k = SpawnOut(b, 0.0, 250.0)
    Utility.Wait(0.5)
    k.StartCombat(Player)
    Utility.Wait(2.0)
    String r = "  " + label + " (level " + k.GetLevel() + ") at 2 s: state " + k.GetCombatState() + ", targets the player " + (k.GetCombatTarget() == Player) + ", hostile " + k.IsHostileToActor(Player) + ", distance " + Num(k.GetDistance(Player))
    Float start = Now()
    Int hits = 0
    Float last = Health(Player)
    While !Player.HasSpell(d) && Now() - start < 90.0
        Utility.Wait(0.5)
        Float h = Health(Player)
        If h < last - 0.5
            hits += 1
        EndIf
        If h < Player.GetActorValueMax("Health") - 300.0
            Player.RestoreActorValue("Health", 5000.0)
            h = Health(Player)
        EndIf
        last = h
    EndWhile
    r += "; " + hits + " hits in " + Num(Now() - start) + " s, HasSpell(its disease) " + Player.HasSpell(d)
    k.StopCombat()
    Gone(k)
    Player.StopCombatAlarm()
    Utility.Wait(1.0)
    r += "; then GetDisease " + Probe(Player, PS_Disease, PE_Disease)
    Player.RemoveSpell(d)
    Utility.Wait(1.0)
    return r + "; removed: GetDisease " + Probe(Player, PS_Disease, PE_Disease) + "\n"
EndFunction

String Function TestDisease()
    Player.MoveTo(StartMarker, 0.0, 0.0, 0.0, true)
    Utility.Wait(5.0)
    If Player.IsInInterior()
        return "skipped: the run began indoors\n"
    EndIf
    Player.ModActorValue("Health", 1000.0)
    Player.RestoreActorValue("Health", 5000.0)
    String r = "the player: DiseaseResist " + Num(Player.GetActorValue("DiseaseResist")) + ", GetDisease " + Probe(Player, PS_Disease, PE_Disease) + "\n"
    r += Carrier(SkeeverBase, Ataxia, "a skeever (Ataxia)")
    r += Carrier(WolfBase, Rockjoint, "a wolf (Rockjoint)")
    r += Carrier(BearBase, BoneBreak, "a bear (Bone Break Fever)")
    Player.RestoreActorValue("Health", 5000.0)
    Player.ModActorValue("Health", -1000.0)
    return r
EndFunction
