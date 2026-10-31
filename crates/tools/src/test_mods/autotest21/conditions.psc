; Conditions, each read through a probe ability (see Probe in base.psc):
; status-conditions, comparison-conditions, condition-geometry, death,
; condition-variables and combat-conditions' static ones. Probes comparing
; with ProbeValue (a global) are scanned by setting it.

; The first threshold in steps at which a ">= ProbeValue" probe stops
; holding, as text.
String Function Scan(Actor a, Spell s, MagicEffect e, Float start, Float stop, Float step)
    String r = ""
    Float v = start
    String last = ""
    While v <= stop
        ProbeValue.SetValue(v)
        String now = Probe(a, s, e)
        If now != last
            r += " " + v + ":" + now
            last = now
        EndIf
        v += step
    EndWhile
    return r
EndFunction

; The values at which an "== ProbeValue" probe holds.
String Function Equals(Actor a, Spell s, MagicEffect e, Float start, Float stop, Float step)
    String r = ""
    Float v = start
    While v <= stop
        ProbeValue.SetValue(v)
        If Probe(a, s, e) == "yes"
            r += " " + v
        EndIf
        v += step
    EndWhile
    If r == ""
        return " none"
    EndIf
    return r
EndFunction

String Function TestPlatformConditions()
    String r = "IsWin32 " + Probe(Player, PS_Win32, PE_Win32) + ", IsGoreDisabled " + Probe(Player, PS_Gore, PE_Gore)
    Game.ForceFirstPerson()
    Utility.Wait(1.0)
    r += ", IsPC1stPerson in first person " + Probe(Player, PS_FirstPerson, PE_FirstPerson)
    Game.ForceThirdPerson()
    Utility.Wait(1.0)
    r += ", in third " + Probe(Player, PS_FirstPerson, PE_FirstPerson) + "\n"
    Int level = Player.GetLevel()
    Float cost = Game.GetGameSettingFloat("fXPLevelUpBase") + level * Game.GetGameSettingFloat("fXPLevelUpMult")
    ProbeValue.SetValue(cost)
    r += "GetXPForNextLevel == " + cost + " (base + level " + level + " x mult): " + Probe(Player, PS_XPNext, PE_XPNext)
    r += "; scan" + Scan(Player, PS_XPNextGE, PE_XPNextGE, 0.0, 1000.0, 25.0) + "\n"
    Game.DisablePlayerControls(true, false, false, false, false, false, false, false, 0)
    Utility.Wait(0.5)
    r += "movement disabled: GetPlayerControlsDisabled(0,0) " + Probe(Player, PS_Controls00, PE_Controls00)
    r += ", (1,0) " + Probe(Player, PS_Controls10, PE_Controls10) + ", (0,1) " + Probe(Player, PS_Controls01, PE_Controls01)
    Game.EnablePlayerControls(true, true, true, true, true, true, true, true, 0)
    Utility.Wait(0.5)
    r += "; enabled: (0,0) " + Probe(Player, PS_Controls00, PE_Controls00) + ", (1,0) " + Probe(Player, PS_Controls10, PE_Controls10) + "\n"
    ProbeValue.SetValue(0.0)
    r += "GetCrimeGold (no faction) on the player == 0: " + Probe(Player, PS_CrimeGold, PE_CrimeGold)
    CrimeFaction.SetCrimeGold(70)
    ProbeValue.SetValue(70.0)
    r += "; with 70 in one faction == 70: " + Probe(Player, PS_CrimeGold, PE_CrimeGold)
    r += ", on an NPC == 70: " + Probe(Alpha, PS_CrimeGold, PE_CrimeGold) + "\n"
    CrimeFaction.SetCrimeGold(0)
    return r
EndFunction

String Function TestItemConditions()
    Actor a = Spawn(NakedBase, 0.0, 300.0)
    a.AddItem(IronSword, 3, true)
    a.EquipItem(IronSword, false, true)
    Utility.Wait(1.0)
    String r = "three iron swords, one equipped: GetKeywordItemCount(WeapTypeSword) ==" + Equals(a, PS_KwCount, PE_KwCount, 0.0, 4.0, 1.0)
    r += ", GetWeaponAnimType ==" + Equals(a, PS_AnimType, PE_AnimType, 0.0, 9.0, 1.0)
    r += ", HasTwoHandedWeaponEquipped " + Probe(a, PS_TwoHanded, PE_TwoHanded) + "\n"
    a.AddItem(IronGreatsword, 1, true)
    a.EquipItem(IronGreatsword, false, true)
    Utility.Wait(1.0)
    r += "a greatsword: GetWeaponAnimType ==" + Equals(a, PS_AnimType, PE_AnimType, 0.0, 9.0, 1.0) + ", HasTwoHandedWeaponEquipped " + Probe(a, PS_TwoHanded, PE_TwoHanded) + "\n"
    a.AddItem(Torch, 1, true)
    a.EquipItem(Torch, false, true)
    a.UnequipItem(IronGreatsword, false, true)
    Utility.Wait(1.0)
    r += "a torch in hand (sheathed): IsActorUsingATorch " + Probe(a, PS_Torch, PE_Torch) + ", IsTorchOut " + Probe(a, PS_TorchOut, PE_TorchOut)
    a.DrawWeapon()
    Utility.Wait(2.0)
    r += "; drawn: IsActorUsingATorch " + Probe(a, PS_Torch, PE_Torch) + ", IsTorchOut " + Probe(a, PS_TorchOut, PE_TorchOut) + "\n"
    Gone(a)
    Actor b = Spawn(NakedBase, 0.0, 300.0)
    b.AddItem(IronCuirass, 1, true)
    b.EquipItem(IronCuirass, false, true)
    b.AddItem(IronShield, 1, true)
    b.EquipItem(IronShield, false, true)
    b.AddItem(IronSword, 1, true)
    b.EquipItem(IronSword, false, true)
    Utility.Wait(1.0)
    r += "iron cuirass (value " + IronCuirass.GetGoldValue() + ") and shield (" + IronShield.GetGoldValue() + "): GetClothingValue >=" + Scan(b, PS_ClothingGE, PE_ClothingGE, 0.0, 200.0, 5.0) + "\n"
    r += "shield sheathed: IsShieldOut " + Probe(b, PS_ShieldOut, PE_ShieldOut)
    b.DrawWeapon()
    Utility.Wait(2.0)
    r += "; drawn " + Probe(b, PS_ShieldOut, PE_ShieldOut) + "\n"
    Gone(b)
    return r
EndFunction

String Function TestActorConditions()
    Actor a = Spawn(NakedBase, 0.0, 300.0)
    Actor w = Spawn(WolfBase, 300.0, 300.0)
    w.SetDontMove(true)
    Utility.Wait(1.0)
    String r = "GetIsCreature: NPC " + Probe(a, PS_Creature, PE_Creature) + ", wolf " + Probe(w, PS_Creature, PE_Creature) + "\n"
    a.SetScale(2.0)
    Utility.Wait(0.5)
    r += "SetScale(2), GetScale native " + a.GetScale() + ": condition >=" + Scan(a, PS_ScaleGE, PE_ScaleGE, 0.5, 3.0, 0.1) + "\n"
    a.SetScale(1.0)
    a.AddSpell(Ataxia, false)
    Utility.Wait(0.5)
    r += "Ataxia added: GetDisease " + Probe(a, PS_Disease, PE_Disease) + "\n"
    a.SetUnconscious(true)
    Utility.Wait(1.0)
    r += "SetUnconscious: GetUnconscious " + Probe(a, PS_Unconscious, PE_Unconscious)
    a.SetUnconscious(false)
    Utility.Wait(1.0)
    ProtectBase.SetEssential(true)
    Actor e = Spawn(ProtectBase, -300.0, 300.0)
    e.Kill(None)
    Utility.Wait(2.0)
    r += "; essential in bleedout (bleeding " + e.IsBleedingOut() + "): GetUnconscious " + Probe(e, PS_Unconscious, PE_Unconscious) + "\n"
    ProtectBase.SetEssential(false)
    a.DamageActorValue("Stamina", a.GetActorValue("Stamina") * 0.6)
    Utility.Wait(0.5)
    r += "stamina at " + a.GetActorValuePercentage("Stamina") + ": GetStaminaPercentage >= 50 " + Probe(a, PS_Stamina50, PE_Stamina50) + ", >= 0.3 " + Probe(a, PS_Stamina03, PE_Stamina03) + ", >= 0.5 " + Probe(a, PS_Stamina05, PE_Stamina05) + "\n"
    a.AddItem(IronSword, 1, true)
    a.ForceActorValue("CarryWeight", 1.0)
    Utility.Wait(1.0)
    r += "NPC over its carry weight (native " + a.IsOverEncumbered() + "): IsOverEncumbered " + Probe(a, PS_OverEnc, PE_OverEnc) + "\n"
    ProtectBase.SetProtected(true)
    Actor p = Spawn(ProtectBase, -300.0, -300.0)
    Utility.Wait(0.5)
    r += "base SetProtected: IsProtected " + Probe(p, PS_Protected, PE_Protected) + ", native " + p.GetActorBase().IsProtected()
    ProtectBase.SetProtected(false)
    Utility.Wait(0.5)
    r += "; cleared " + Probe(p, PS_Protected, PE_Protected) + "\n"
    Gone(a)
    Gone(w)
    Gone(e)
    Gone(p)
    return r
EndFunction

; Factions: Alpha (rank 0) and Beta (rank -1) in Teal; Red lists Green as an
; enemy, Green lists nobody.
String Function TestFactionConditions()
    String r = "SameFaction(Beta, Teal rank -1) on Alpha: " + Probe(Alpha, PS_SameFaction, PE_SameFaction)
    r += ", SameRace(Gamma, Nord with an Orc traits template) on Alpha (Nord): " + Probe(Alpha, PS_SameRace, PE_SameRace) + "\n"
    r += "GetFactionCombatReaction(Red, Green) ==" + Equals(Alpha, PS_ReactionRG, PE_ReactionRG, 0.0, 3.0, 1.0)
    r += ", (Green, Red) ==" + Equals(Alpha, PS_ReactionGR, PE_ReactionGR, 0.0, 3.0, 1.0)
    r += ", (Teal, Teal) ==" + Equals(Alpha, PS_ReactionTT, PE_ReactionTT, 0.0, 3.0, 1.0) + "\n"
    r += "GetFactionRelation(Beta) on Alpha ==" + Equals(Alpha, PS_FactionRelation, PE_FactionRelation, 0.0, 3.0, 1.0)
    r += ", GetFactionRankDifference(Teal, Beta) ==" + Equals(Alpha, PS_RankDiff, PE_RankDiff, -3.0, 3.0, 1.0)
    r += ", GetShouldHelp(Beta) " + Probe(Alpha, PS_ShouldHelp, PE_ShouldHelp) + "\n"
    r += "GetInCellParam(our cell, Beta) on Alpha " + Probe(Alpha, PS_InCellParam, PE_InCellParam) + "\n"
    r += "GetScriptVariable(Alpha, Marker) == 7 on Beta " + Probe(Beta, PS_ScriptVar, PE_ScriptVar)
    r += ", on Alpha itself " + Probe(Alpha, PS_ScriptVar, PE_ScriptVar)
    r += ", a missing variable == 0 on Beta " + Probe(Beta, PS_ScriptVarMissing, PE_ScriptVarMissing) + "\n"
    return r
EndFunction

; Geometry: Alpha faces north at the centre; Beta and Gamma are moved round.
String Function TestGeometry()
    Alpha.SetDontMove(true)
    Beta.SetDontMove(true)
    Gamma.SetDontMove(true)
    Alpha.MoveTo(CellCenter, 0.0, 0.0, 0.0, false)
    Alpha.SetAngle(0.0, 0.0, 0.0)
    String r = ""
    ; Beta to the east (Alpha's right), then south, west, north.
    Float[] xs = new Float[4]
    Float[] ys = new Float[4]
    xs[0] = 300.0
    ys[0] = 0.0
    xs[1] = 0.0
    ys[1] = -300.0
    xs[2] = -300.0
    ys[2] = 0.0
    xs[3] = 0.0
    ys[3] = 300.0
    String[] names = new String[4]
    names[0] = "east (right)"
    names[1] = "south (behind)"
    names[2] = "west (left)"
    names[3] = "north (ahead)"
    Int i = 0
    While i < 4
        Beta.MoveTo(CellCenter, xs[i], ys[i], 0.0, false)
        Beta.SetAngle(0.0, 0.0, 90.0)
        Utility.Wait(1.0)
        r += "Beta " + names[i] + ", facing east: native GetHeadingAngle " + Num(Alpha.GetHeadingAngle(Beta)) + "; GetHeadingAngle >=" + Scan(Alpha, PS_Heading, PE_Heading, -180.0, 360.0, 45.0)
        r += "; GetRelativeAngle >=" + Scan(Alpha, PS_RelAngle, PE_RelAngle, -180.0, 360.0, 45.0) + "\n"
        i += 1
    EndWhile
    Beta.MoveTo(CellCenter, 300.0, 0.0, 0.0, false)
    Utility.Wait(1.0)
    r += "Beta at GetDistance " + Num(Alpha.GetDistance(Beta)) + ": GetWithinDistance(Beta, 300) " + Probe(Alpha, PS_Within300, PE_Within300)
    Beta.MoveTo(Alpha, 300.0, 0.0, 0.0, false)
    Utility.Wait(1.0)
    r += "; moved to exactly 300 from Alpha (" + Num(Alpha.GetDistance(Beta)) + "): " + Probe(Alpha, PS_Within300, PE_Within300)
    Beta.MoveTo(Alpha, 299.0, 0.0, 0.0, false)
    Utility.Wait(1.0)
    r += "; 299 (" + Num(Alpha.GetDistance(Beta)) + "): " + Probe(Alpha, PS_Within300, PE_Within300)
    Beta.MoveTo(Alpha, 0.0, 0.0, 300.0, false)
    Utility.Wait(0.2)
    r += "; 300 straight up (" + Num(Alpha.GetDistance(Beta)) + "): " + Probe(Alpha, PS_Within300, PE_Within300) + "\n"
    Beta.MoveTo(Alpha, 200.0, 0.0, 0.0, false)
    Gamma.MoveTo(Alpha, -200.0, 0.0, 0.0, false)
    Utility.Wait(1.0)
    r += "Beta and Gamma 200 either side: IsCloserToAThanB(Beta, Gamma) " + Probe(Alpha, PS_Closer, PE_Closer)
    Gamma.MoveTo(Alpha, -250.0, 0.0, 0.0, false)
    Utility.Wait(1.0)
    r += "; Gamma at 250: " + Probe(Alpha, PS_Closer, PE_Closer) + "\n"
    r += "GetLineOfSight(Beta) in the open " + Probe(Alpha, PS_LOS, PE_LOS) + ", native HasLOS " + Alpha.HasLOS(Beta)
    Beta.MoveTo(Alpha, 0.0, -200.0, 0.0, false)
    Utility.Wait(1.0)
    r += "; Beta behind Alpha: " + Probe(Alpha, PS_LOS, PE_LOS) + ", HasLOS " + Alpha.HasLOS(Beta) + "\n"
    Actor s = Spawn(NakedBase, 123.0, 456.0)
    s.SetDontMove(true)
    Utility.Wait(1.0)
    Float x0 = s.GetPositionX()
    s.MoveTo(CellCenter, -400.0, 456.0, 0.0, false)
    Utility.Wait(1.0)
    ProbeValue.SetValue(x0)
    r += "a placed actor moved from x " + Num(x0) + " to " + Num(s.GetPositionX()) + ": GetStartingPos(X) == the first " + Probe(s, PS_StartX, PE_StartX)
    ProbeValue.SetValue(s.GetPositionX())
    r += ", == the second " + Probe(s, PS_StartX, PE_StartX)
    ProbeValue.SetValue(CellCenter.GetPositionX())
    r += ", == the marker it was placed at " + Probe(s, PS_StartX, PE_StartX) + "\n"
    Gone(s)
    Alpha.MoveTo(CellCenter, -600.0, -600.0, 0.0, false)
    Beta.MoveTo(CellCenter, -300.0, -600.0, 0.0, false)
    Gamma.MoveTo(CellCenter, 0.0, -600.0, 0.0, false)
    return r
EndFunction

; Death: Victim (a placed NPC of the plugin) killed by Alpha, then read
; through probes run on Victim from Alpha.
String Function TestDeathConditions()
    Victim.MoveTo(CellCenter, 400.0, -400.0, 0.0, false)
    Utility.Wait(0.5)
    Victim.Kill(Alpha)
    Float t0 = Utility.GetCurrentGameTime()
    Float r0 = Now()
    Utility.Wait(1.0)
    String r = "Victim killed by Alpha (GetKiller " + Id(Victim.GetKiller()) + ", Alpha " + Id(Alpha) + ")\n"
    r += "IsKiller(Victim) on Alpha " + Probe(Alpha, PS_KillerOf, PE_KillerOf)
    r += ", IsKiller(Alpha) run on Victim " + Probe(Alpha, PS_KillerRev, PE_KillerRev) + "\n"
    r += "IsKillerObject(list of Victim's base) on Alpha " + Probe(Alpha, PS_KillerBase, PE_KillerBase)
    r += ", (list of Victim's reference) " + Probe(Alpha, PS_KillerRef, PE_KillerRef) + "\n"
    r += "GetCauseofDeath on Victim ==" + Equals(Alpha, PS_Cause, PE_Cause, -2.0, 12.0, 1.0) + "\n"
    Utility.Wait(20.0)
    Float game = (Utility.GetCurrentGameTime() - t0) * 24.0
    r += "after " + Num(Now() - r0) + " s (" + Num(game) + " game hours): GetTimeDead >="
    Float[] t = new Float[9]
    t[0] = 0.001
    t[1] = 0.01
    t[2] = 0.05
    t[3] = 0.1
    t[4] = 0.3
    t[5] = 1.0
    t[6] = 10.0
    t[7] = 100.0
    t[8] = 1000.0
    Int i = 0
    While i < 9
        ProbeValue.SetValue(t[i])
        r += " " + t[i] + ":" + Probe(Alpha, PS_TimeDead, PE_TimeDead)
        i += 1
    EndWhile
    r += "\n"
    Actor follower = Spawn(NakedBase, 0.0, 300.0)
    follower.SetPlayerTeammate(true, true)
    Actor v2 = Spawn(NakedBase, 300.0, 300.0)
    v2.Kill(follower)
    Utility.Wait(1.0)
    r += "killed by the player's teammate: GetKiller is " + Id(v2.GetKiller()) + " (teammate " + Id(follower) + ", player " + Id(Player) + ")\n"
    Victim.Resurrect()
    follower.SetPlayerTeammate(false, false)
    Gone(follower)
    Gone(v2)
    return r
EndFunction
