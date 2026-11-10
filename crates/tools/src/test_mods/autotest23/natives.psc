; Natives and settings, in the test cell (whose location is Whiterun Hold
; in this batch).

; combat-fleeing, npc-healing, detection: settings the open items name, as
; the game has them (those Skyrim.esm lacks come from the engine).
String Function Setting(String name)
    return " " + name + " " + Num(Game.GetGameSettingFloat(name)) + ";"
EndFunction

String Function TestGameSettings()
    String r = "floats:"
    r += Setting("fFleeIsSafeTimer") + Setting("fFleeDistance") + Setting("fFleeDoneDistance")
    r += Setting("fCombatFleeDistance") + Setting("fCombatFleeDistanceInterior") + Setting("fCombatFleeDistanceExterior")
    r += Setting("fCombatFleeWaitTime") + Setting("fCombatFleeBoostConfidence") + Setting("fCombatFleeBoostConfidenceTargetRadius")
    r += Setting("fCombatDialogueFleeMinElapsedTime") + Setting("fCombatDialogueFleeMaxElapsedTime") + Setting("fCombatDialogueFleeDistanceMult")
    r += Setting("fCombatLowFleeingTargetHitPercent") + Setting("fCombatDetectionFleeingLostRemoveTime") + Setting("fCombatDetectionLostRemoveTime")
    r += Setting("fCombatConfidenceModifierMin") + Setting("fCombatConfidenceModifierMax") + Setting("fCombatStrengthUpdateTime")
    r += Setting("fCombatThreatRatioUpdateTime") + Setting("fCombatGroupCombatStrengthUpdateTime")
    r += Setting("fCombatRestoreHealthPercentMin") + Setting("fCombatRestoreHealthPercentMax") + Setting("fCombatRestoreHealthRestrictTime")
    r += Setting("fCombatRestoreStopCastThreshold") + Setting("fCombatItemRestoreTimer") + Setting("fCombatHealthRegenRateMult")
    r += "\nints: iAICombatRestoreHealthPercentage " + Game.GetGameSettingInt("iAICombatRestoreHealthPercentage")
    r += ", iAICombatRestoreMagickaPercentage " + Game.GetGameSettingInt("iAICombatRestoreMagickaPercentage")
    r += ", iFriendHitCombatAllowed " + Game.GetGameSettingInt("iFriendHitCombatAllowed") + "\n"
    return r
EndFunction

; journal / natives: GameTimeToString on this save (autotest21 read
; 07/16/0201 for 0 and 1.5 on a save in its first days).
String Function TestGameTimeToString()
    Float now = Utility.GetCurrentGameTime()
    String r = "GameDaysPassed " + Num(GameDaysPassed.GetValue()) + ", GetCurrentGameTime " + Num(now) + ", GameHour " + Num(GameHour.GetValue())
    r += "; GameTimeToString: 0 " + Utility.GameTimeToString(0.0)
    r += ", 1 " + Utility.GameTimeToString(1.0)
    r += ", 1.5 " + Utility.GameTimeToString(1.5)
    r += ", 31 " + Utility.GameTimeToString(31.0)
    r += ", 365 " + Utility.GameTimeToString(365.0)
    r += ", now " + Utility.GameTimeToString(now)
    r += ", now + 1 " + Utility.GameTimeToString(now + 1.0) + "\n"
    return r
EndFunction

; relationships: HasParentRelationship and HasFamilyRelationship with no
; actor given.
String Function Family(Actor a, String label)
    return label + ": HasParentRelationship(None) " + a.HasParentRelationship(None) + ", HasFamilyRelationship(None) " + a.HasFamilyRelationship(None)
EndFunction

String Function TestParentNone()
    Actor g = Spawn(GerdurBase, 0.0, 300.0)
    Actor f = Spawn(FrodnarBase, 150.0, 300.0)
    Actor h = Spawn(HodBase, -150.0, 300.0)
    Actor n = Spawn(NakedBase, 0.0, 450.0)
    Utility.Wait(1.0)
    String r = Family(g, "Gerdur (mother of Frodnar, wife of Hod)") + "; " + Family(f, "Frodnar (son)") + "\n"
    r += Family(h, "Hod (father, husband)") + "; " + Family(n, "an NPC with no relationships") + "\n"
    Gone(g)
    Gone(f)
    Gone(h)
    Gone(n)
    return r
EndFunction

; crime: what makes IsGuard true besides the faction or class (autotest21:
; neither alone did, without a crime faction).
String Function GuardLine(ActorBase b, String label)
    Actor a = Spawn(b, 0.0, 300.0)
    a.SetDontMove(true)
    Utility.Wait(1.0)
    String r = "  " + label + ": IsGuard " + a.IsGuard() + ", crime faction " + Id(a.GetCrimeFaction()) + ", class " + Id(a.GetActorBase().GetClass()) + "\n"
    Gone(a)
    return r
EndFunction

String Function TestIsGuard()
    String r = "(Whiterun's crime faction " + Id(CrimeWhiterun) + ")\n"
    r += GuardLine(GuardWhiterunBase, "a Whiterun guard (vanilla base)")
    r += GuardLine(GuardCopyBase, "a copy of it")
    r += GuardLine(GuardCopyNoCrimeBase, "the copy without its crime faction")
    r += GuardLine(GuardClassBase, "Guard class, no crime faction")
    r += GuardLine(GuardClassCrimeBase, "Guard class, Whiterun's crime faction")
    r += GuardLine(GuardFactionBase, "IsGuardFaction, no crime faction")
    r += GuardLine(GuardFactionCrimeBase, "IsGuardFaction, Whiterun's crime faction")
    r += GuardLine(CrimeGuardBase, "Guard class, member and crime faction ours (Arrest)")
    r += GuardLine(AosGuardBase, "Guard class, member and crime faction ours (Arrest, Attack on Sight)")
    return r
EndFunction

; created-refs: what a script still holding a deleted reference sees, and
; Delete on a plugin reference.
String Function TestDeleteHeld()
    ObjectReference b = Put(Barrel, 0.0, 200.0, 0.0)
    Utility.Wait(1.0)
    b.Delete()
    Utility.Wait(1.0)
    String r = "a created barrel, deleted and still held: IsDeleted " + b.IsDeleted() + ", disabled " + b.IsDisabled() + ", 3D " + b.Is3DLoaded() + ", x " + Num(b.GetPositionX())
    b.MoveTo(CellCenter, 100.0, 200.0, 0.0, false)
    Utility.Wait(0.5)
    r += "; MoveTo: x " + Num(b.GetPositionX()) + ", 3D " + b.Is3DLoaded()
    b.Enable(false)
    Utility.Wait(0.5)
    r += "; Enable: disabled " + b.IsDisabled() + ", 3D " + b.Is3DLoaded()
    b.AddItem(Gold, 5, true)
    r += "; AddItem 5 gold: count " + b.GetItemCount(Gold) + "\n"
    r += "a plugin barrel before Delete: disabled " + DeleteMe.IsDisabled() + ", 3D " + DeleteMe.Is3DLoaded()
    DeleteMe.Delete()
    Utility.Wait(1.0)
    r += "; after: IsDeleted " + DeleteMe.IsDeleted() + ", disabled " + DeleteMe.IsDisabled() + ", 3D " + DeleteMe.Is3DLoaded() + ", x " + Num(DeleteMe.GetPositionX())
    DeleteMe.Enable(false)
    Utility.Wait(1.0)
    r += "; Enable: disabled " + DeleteMe.IsDisabled() + ", 3D " + DeleteMe.Is3DLoaded() + ", IsDeleted " + DeleteMe.IsDeleted() + "\n"
    return r
EndFunction

; magic: whether Spells Learned counts a scripted AddSpell.
String Function TestSpellsLearned()
    Spell s = None
    If !Player.HasSpell(FireboltSpell)
        s = FireboltSpell
    ElseIf !Player.HasSpell(FearSpell)
        s = FearSpell
    ElseIf !Player.HasSpell(CalmSpell)
        s = CalmSpell
    ElseIf !Player.HasSpell(FlameCloakSpell)
        s = FlameCloakSpell
    EndIf
    If s == None
        return "skipped: the player knows every spell tried\n"
    EndIf
    Int before = Game.QueryStat("Spells Learned")
    Player.AddSpell(s, false)
    Utility.Wait(1.0)
    Int mid = Game.QueryStat("Spells Learned")
    Player.AddSpell(WhFireSpell, false)
    Utility.Wait(1.0)
    String r = "Spells Learned " + before + "; AddSpell of a vanilla spell " + Id(s) + ": " + mid + "; of a plugin spell: " + Game.QueryStat("Spells Learned")
    Player.RemoveSpell(s)
    Player.RemoveSpell(WhFireSpell)
    Utility.Wait(1.0)
    return r + "; both removed: " + Game.QueryStat("Spells Learned") + "\n"
EndFunction

; query-natives: locations in a cell that has one (ours: Whiterun Hold), and
; GetCrimeGold with no faction inside a hold (autotest22: in a cell with no
; location it didn't add up the bounties).
String Function Loc(Location l)
    return Id(l)
EndFunction

String Function TestCellLocation()
    ObjectReference m = Player.PlaceAtMe(XMarker, 1, true, false)
    ObjectReference s = Put(IronSword, 0.0, 150.0, 20.0)
    Utility.Wait(1.0)
    String r = "(Whiterun Hold " + Id(WhiterunHold) + ", Whiterun " + Id(WhiterunCity) + ", Bleak Falls " + Id(BleakFalls) + ")\n"
    r += "the player: current " + Loc(Player.GetCurrentLocation()) + ", editor " + Loc(Player.GetEditorLocation())
    r += "; a created marker: current " + Loc(m.GetCurrentLocation()) + ", editor " + Loc(m.GetEditorLocation())
    r += "; a plugin marker (CellCenter): current " + Loc(CellCenter.GetCurrentLocation()) + ", editor " + Loc(CellCenter.GetEditorLocation())
    r += "; a plugin actor (Alpha): editor " + Loc(Alpha.GetEditorLocation()) + "\n"
    r += "a created sword on the floor: current " + Loc(s.GetCurrentLocation()) + ", editor " + Loc(s.GetEditorLocation())
    Player.AddItem(s, 1, true)
    Utility.Wait(1.0)
    r += "; given to the player by AddItem(ref): current " + Loc(s.GetCurrentLocation()) + ", editor " + Loc(s.GetEditorLocation()) + ", the player holds " + Player.GetItemCount(IronSword) + "\n"
    r += "IsLoaded: Whiterun Hold " + WhiterunHold.IsLoaded() + ", Whiterun " + WhiterunCity.IsLoaded() + ", Bleak Falls " + BleakFalls.IsLoaded() + "\n"
    Int wr = CrimeWhiterun.GetCrimeGold()
    Int rift = CrimeRift.GetCrimeGold()
    CrimeWhiterun.SetCrimeGold(7)
    CrimeRift.SetCrimeGold(3)
    r += "bounties Whiterun 7, the Rift 3 (were " + wr + ", " + rift + "): GetCrimeGold with no faction on the player =="
    Int[] vals = new Int[4]
    vals[0] = 0
    vals[1] = 3
    vals[2] = 7
    vals[3] = 10
    Int i = 0
    While i < 4
        ProbeValue.SetValue(vals[i] as Float)
        r += " " + vals[i] + ":" + Probe(Player, PS_CrimeGold, PE_CrimeGold)
        i += 1
    EndWhile
    CrimeWhiterun.SetCrimeGold(wr)
    CrimeRift.SetCrimeGold(rift)
    Player.RemoveItem(IronSword, 1, true, None)
    Gone(m)
    return r + "\n"
EndFunction
