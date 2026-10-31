; Natives: query-natives, math-natives, remaining-natives, actor-natives,
; created-refs, outfits, formlists, factions, protection and crime's
; runtime faction changes. All in the test cell.

; query-natives: IsUnique through templates, GetClass of a leveled stats
; template, GetEditorLocation of a created reference.
String Function TestQueryNatives()
    String r = "IsUnique: plain unique " + UniqueBase.IsUnique()
    r += ", non-unique child of a unique traits template " + TplUniqueChild.IsUnique()
    r += ", unique child of a non-unique template " + TplPlainUniqueChild.IsUnique() + "\n"
    r += "GetClass: leveled stats template base " + Id(LvlStatsChild.GetClass())
    Actor a = Spawn(LvlStatsChild, 0.0, 300.0)
    r += ", placed actor's leveled base " + Id(a.GetLeveledActorBase().GetClass())
    r += ", Citizen is " + Id(CitizenClass) + "\n"
    ObjectReference o = Put(XMarker, 0.0, 0.0, 0.0)
    r += "GetEditorLocation: created marker " + Id(o.GetEditorLocation())
    r += ", player " + Id(Player.GetEditorLocation())
    r += ", GetCurrentLocation created marker " + Id(o.GetCurrentLocation()) + "\n"
    r += "Bleak Falls Boss alive " + BleakFalls.GetRefTypeAliveCount(BossType)
    r += ", Whiterun hold Boss alive " + WhiterunHold.GetRefTypeAliveCount(BossType)
    r += ", Bleak Falls dead " + BleakFalls.GetRefTypeDeadCount(BossType) + "\n"
    r += "hold HasRefType(Boss) " + WhiterunHold.HasRefType(BossType)
    r += ", Bleak Falls " + BleakFalls.HasRefType(BossType) + "\n"
    r += "Whiterun HasCommonParent(hold) " + WhiterunCity.HasCommonParent(WhiterunHold, None)
    r += ", Bleak Falls HasCommonParent(Whiterun) " + BleakFalls.HasCommonParent(WhiterunCity, None)
    r += ", Whiterun IsChild(hold) " + WhiterunCity.IsChild(WhiterunHold) + "\n"
    Gone(a)
    Gone(o)
    return r
EndFunction

; math-natives: out of domain, poles, rounding.
String Function TestMathNatives()
    String r = "asin(2) " + (Math.asin(2.0) as String) + ", acos(-2) " + (Math.acos(-2.0) as String)
    r += ", sqrt(-1) " + (Math.sqrt(-1.0) as String) + ", tan(90) " + (Math.tan(90.0) as String)
    r += ", tan(270) " + (Math.tan(270.0) as String) + "\n"
    r += "pow(0,-1) " + (Math.pow(0.0, -1.0) as String) + ", pow(-8,1/3) " + (Math.pow(-8.0, 0.3333) as String)
    r += ", Floor(-0.5) " + Math.Floor(-0.5) + ", Ceiling(-0.5) " + Math.Ceiling(-0.5)
    r += ", Floor(2.9999999) " + Math.Floor(2.9999999) + "\n"
    r += "asin(1) " + (Math.asin(1.0) as String) + ", atan(1) " + (Math.atan(1.0) as String)
    r += ", sin(30) " + (Math.sin(30.0) as String) + ", 1.0/3 as String " + ((1.0 / 3.0) as String)
    r += ", 1e9 as Int " + ((1000000000.0 * 4.0) as Int) + "\n"
    return r
EndFunction

; remaining-natives: containers, sizes, statistics, quests, references.
String Function TestContainerCounts()
    ObjectReference c = Put(EmptyChest, 200.0, 0.0, 0.0)
    String r = "empty chest: IsContainerEmpty " + c.IsContainerEmpty() + ", GetAllItemsCount " + c.GetAllItemsCount()
    c.AddItem(Gold, 10, true)
    r += "; 10 gold: empty " + c.IsContainerEmpty() + ", count " + c.GetAllItemsCount()
    c.AddItem(IronSword, 3, true)
    c.AddItem(IngotIron, 2, true)
    r += "; + 3 swords, 2 ingots: count " + c.GetAllItemsCount() + "\n"
    Gone(c)
    return r
EndFunction

String Function TestWornKeyword()
    Actor a = Spawn(NakedBase, 0.0, 300.0)
    a.AddItem(IronSword, 1, true)
    a.EquipItem(IronSword, false, true)
    a.AddItem(IronCuirass, 1, true)
    a.EquipItem(IronCuirass, false, true)
    Utility.Wait(1.0)
    String r = "sword equipped " + a.IsEquipped(IronSword) + ": WornHasKeyword(WeapTypeSword) " + a.WornHasKeyword(WeapTypeSword)
    r += ", (ArmorCuirass) " + a.WornHasKeyword(ArmorCuirassKw) + "\n"
    a.ForceActorValue("CarryWeight", 0.0)
    Utility.Wait(0.5)
    r += "carry weight 0 holding 9 + 30: IsOverEncumbered " + a.IsOverEncumbered() + "\n"
    Gone(a)
    return r
EndFunction

; NPC encumbrance at the limit: an NPC with only an iron sword (9).
String Function TestEncumbranceEdge()
    Actor a = Spawn(NakedBase, 0.0, 300.0)
    a.AddItem(IronSword, 1, true)
    String r = "an iron sword carried, CarryWeight 9: "
    a.ForceActorValue("CarryWeight", 9.0)
    Utility.Wait(0.5)
    r += "over " + a.IsOverEncumbered()
    a.ForceActorValue("CarryWeight", 8.9)
    Utility.Wait(0.5)
    r += "; 8.9: over " + a.IsOverEncumbered()
    a.ForceActorValue("CarryWeight", 9.1)
    Utility.Wait(0.5)
    r += "; 9.1: over " + a.IsOverEncumbered() + "\n"
    Gone(a)
    return r
EndFunction

String Function TestDimensions()
    ObjectReference o = Put(Barrel, 0.0, 400.0, 0.0)
    Utility.Wait(0.5)
    String r = "barrel: width " + o.GetWidth() + ", length " + o.GetLength() + ", height " + o.GetHeight() + ", scale " + o.GetScale()
    o.SetScale(2.0)
    Utility.Wait(0.5)
    r += "; SetScale(2): width " + o.GetWidth() + ", height " + o.GetHeight() + ", scale " + o.GetScale()
    o.SetScale(1.0)
    o.SetAngle(0.0, 0.0, 45.0)
    Utility.Wait(0.5)
    r += "; turned 45: width " + o.GetWidth() + ", length " + o.GetLength() + "\n"
    r += "player scale " + Player.GetScale() + ", height " + Player.GetHeight() + "\n"
    Actor a = Spawn(NakedBase, 0.0, 300.0)
    r += "NPC scale " + a.GetScale() + ", height " + a.GetHeight()
    a.SetScale(2.0)
    Utility.Wait(0.5)
    r += "; SetScale(2): scale " + a.GetScale() + ", height " + a.GetHeight() + "\n"
    Gone(a)
    Gone(o)
    return r
EndFunction

String Function TestDeadCount()
    String r = "dead count " + DeadCountBase.GetDeadCount()
    Actor a = Spawn(DeadCountBase, 0.0, 300.0)
    a.Kill(None)
    Utility.Wait(1.0)
    r += ", one killed " + DeadCountBase.GetDeadCount()
    a.Resurrect()
    Utility.Wait(1.0)
    r += ", resurrected " + DeadCountBase.GetDeadCount() + " (IsDead " + a.IsDead() + ")"
    a.Kill(None)
    Utility.Wait(1.0)
    r += ", killed again " + DeadCountBase.GetDeadCount()
    Gone(a)
    Utility.Wait(1.0)
    r += ", deleted " + DeadCountBase.GetDeadCount() + "\n"
    r += "gold values: restore health potion " + PotionHealth.GetGoldValue() + ", torch " + Torch.GetGoldValue()
    r += ", sweet roll " + SweetRoll.GetGoldValue() + "\n"
    return r
EndFunction

String Function TestQuestStates()
    String r = "Start: " + SideQuest.Start()
    r += ", running " + SideQuest.IsRunning() + ", starting " + SideQuest.IsStarting()
    SideQuest.SetStage(10)
    SideQuest.Stop()
    r += "; Stop: at once running " + SideQuest.IsRunning() + ", stopping " + SideQuest.IsStopping() + ", stopped " + SideQuest.IsStopped()
    Utility.Wait(1.0)
    r += "; 1 s on running " + SideQuest.IsRunning() + ", stopped " + SideQuest.IsStopped() + ", stage " + SideQuest.GetStage() + "\n"
    SideQuest.Start()
    SideQuest.SetStage(10)
    SideQuest.Reset()
    Utility.Wait(1.0)
    r += "started, stage 10, Reset: running " + SideQuest.IsRunning() + ", stage " + SideQuest.GetStage() + ", stage 10 done " + SideQuest.IsStageDone(10) + "\n"
    SideQuest.Stop()
    return r
EndFunction

String Function Test3DLoaded()
    ObjectReference o = CellCenter.PlaceAtMe(Barrel, 1, false, false)
    String r = "PlaceAtMe: Is3DLoaded at once " + o.Is3DLoaded()
    Utility.Wait(0.1)
    r += ", after 0.1 s " + o.Is3DLoaded()
    o.Disable(false)
    r += "; Disable: at once " + o.Is3DLoaded()
    Utility.Wait(0.5)
    r += ", 0.5 s on " + o.Is3DLoaded()
    o.Delete()
    r += "; Delete: IsDeleted at once " + o.IsDeleted()
    Utility.Wait(0.5)
    r += ", 0.5 s on " + o.IsDeleted() + ", still named (GetFormID " + o.GetFormID() + ")\n"
    ObjectReference p = CellCenter.PlaceAtMe(Barrel, 1, false, true)
    r += "placed initially disabled: Is3DLoaded " + p.Is3DLoaded() + ", IsDisabled " + p.IsDisabled() + "\n"
    Gone(p)
    return r
EndFunction

String Function TestSoundIds()
    Int a = UISound.Play(Player)
    Int b = UISound.Play(Player)
    Sound.StopInstance(a)
    Sound.StopInstance(b)
    Utility.Wait(0.5)
    Int c = UISound.Play(Player)
    Sound.StopInstance(c)
    return "Play ids: " + a + ", " + b + ", after stopping both " + c + "\n"
EndFunction

String Function TestReferenceSetters()
    ObjectReference d = DoorRef
    String r = "door open state " + d.GetOpenState()
    (d as VrmAt21RefLog).Take()
    d.SetOpen(true)
    Utility.Wait(2.0)
    r += "; SetOpen(true): state " + d.GetOpenState() + ", events " + (d as VrmAt21RefLog).Take()
    d.SetOpen(false)
    Utility.Wait(2.0)
    r += "; SetOpen(false): state " + d.GetOpenState() + ", events " + (d as VrmAt21RefLog).Take() + "\n"
    ObjectReference w = WebRef
    r += "web stage " + w.GetCurrentDestructionStage()
    w.SetDestroyed(true)
    Utility.Wait(1.0)
    r += "; SetDestroyed: stage " + w.GetCurrentDestructionStage() + ", disabled " + w.IsDisabled() + ", events " + (w as VrmAt21RefLog).Take()
    w.SetDestroyed(false)
    Utility.Wait(1.0)
    r += "; SetDestroyed(false): stage " + w.GetCurrentDestructionStage() + ", events " + (w as VrmAt21RefLog).Take() + "\n"
    r += "encounter levels (easy..very hard): " + CellCenter.CalculateEncounterLevel(0) + " " + CellCenter.CalculateEncounterLevel(1) + " " + CellCenter.CalculateEncounterLevel(2) + " " + CellCenter.CalculateEncounterLevel(3) + " " + CellCenter.CalculateEncounterLevel(4)
    r += " at player level " + Player.GetLevel() + "\n"
    return r
EndFunction

String Function TestDestruction()
    ObjectReference w = WebRef2
    (w as VrmAt21RefLog).Take()
    String r = "web: stage " + w.GetCurrentDestructionStage()
    Int i = 0
    While i < 6
        w.DamageObject(5.0)
        Utility.Wait(0.5)
        r += "; +5 damage: stage " + w.GetCurrentDestructionStage()
        i += 1
    EndWhile
    r += "; events " + (w as VrmAt21RefLog).Take()
    w.ClearDestruction()
    Utility.Wait(0.5)
    r += "; ClearDestruction: stage " + w.GetCurrentDestructionStage() + ", events " + (w as VrmAt21RefLog).Take()
    w.DamageObject(1000.0)
    Utility.Wait(0.5)
    r += "; 1000 damage: stage " + w.GetCurrentDestructionStage() + ", disabled " + w.IsDisabled() + ", events " + (w as VrmAt21RefLog).Take() + "\n"
    return r
EndFunction

String Function TestActorState()
    Actor a = Spawn(NakedBase, 0.0, 300.0)
    a.AddItem(IronCuirass, 1, true)
    a.EquipItem(IronCuirass, false, true)
    Utility.Wait(0.5)
    a.Kill(None)
    Utility.Wait(2.0)
    String r = "killed wearing a cuirass: equipped " + a.IsEquipped(IronCuirass)
    a.Resurrect()
    Utility.Wait(2.0)
    r += "; Resurrect: dead " + a.IsDead() + ", equipped " + a.IsEquipped(IronCuirass) + ", count " + a.GetItemCount(IronCuirass) + "\n"
    a.EquipSpell(FlamesSpell, 1)
    a.AddItem(IronSword, 1, true)
    a.EquipItem(IronSword, false, true)
    Utility.Wait(0.5)
    r += "Flames readied right? " + Id(a.GetEquippedSpell(1)) + " left " + Id(a.GetEquippedSpell(0)) + ", sword " + a.IsEquipped(IronSword)
    a.UnequipAll()
    Utility.Wait(1.0)
    r += "; UnequipAll: spells right " + Id(a.GetEquippedSpell(1)) + " left " + Id(a.GetEquippedSpell(0)) + ", sword " + a.IsEquipped(IronSword) + ", cuirass " + a.IsEquipped(IronCuirass) + "\n"
    a.EquipItem(IronCuirass, false, true)
    Utility.Wait(0.5)
    a.UnequipItemSlot(32)
    Utility.Wait(0.5)
    r += "UnequipItemSlot(32): cuirass " + a.IsEquipped(IronCuirass)
    a.EquipItem(IronCuirass, false, true)
    Utility.Wait(0.5)
    a.UnequipItemSlot(2)
    Utility.Wait(0.5)
    r += "; (2): cuirass " + a.IsEquipped(IronCuirass) + "\n"
    a.SetDoingFavor(true)
    r += "SetDoingFavor: IsDoingFavor " + a.IsDoingFavor() + "\n"
    Gone(a)
    return r
EndFunction

String Function TestHostility()
    Actor a = Spawn(RedBase, 0.0, 300.0)
    Actor b = Spawn(GreenBase, 300.0, 300.0)
    a.SetDontMove(true)
    b.SetDontMove(true)
    Utility.Wait(1.0)
    String r = "Red and Green (enemy factions): hostile " + a.IsHostileToActor(b) + "/" + b.IsHostileToActor(a)
    r += ", GetFactionReaction " + a.GetFactionReaction(b) + "/" + b.GetFactionReaction(a)
    b.Kill(None)
    Utility.Wait(1.0)
    r += "; Green dead: hostile " + a.IsHostileToActor(b) + "\n"
    a.StopCombat()
    Gone(a)
    Gone(b)
    return r
EndFunction

; ObjectReference.Reset on a plugin chest moved, emptied, locked, disabled.
String Function TestReset()
    ObjectReference c = ResetChest
    String r = "chest: iron ingots " + c.GetItemCount(IngotIron) + ", locked " + c.IsLocked() + ", x " + Num(c.GetPositionX())
    c.RemoveItem(IngotIron, 1, true, None)
    c.AddItem(Gold, 5, true)
    c.Lock(true, false)
    c.MoveTo(CellCenter, 100.0, -300.0, 0.0, false)
    c.SetActorOwner(NakedBase)
    c.BlockActivation(true)
    Utility.Wait(0.5)
    r += "; changed: ingots " + c.GetItemCount(IngotIron) + ", gold " + c.GetItemCount(Gold) + ", locked " + c.IsLocked() + ", x " + Num(c.GetPositionX())
    c.Reset(None)
    Utility.Wait(1.0)
    r += "; Reset: ingots " + c.GetItemCount(IngotIron) + ", gold " + c.GetItemCount(Gold) + ", locked " + c.IsLocked() + ", x " + Num(c.GetPositionX())
    r += ", owner " + Id(c.GetActorOwner()) + ", activation blocked " + c.IsActivationBlocked() + "\n"
    c.Disable(false)
    c.Reset(None)
    Utility.Wait(1.0)
    r += "disabled, Reset: disabled " + c.IsDisabled() + "\n"
    c.Enable(false)
    c.BlockActivation(false)
    return r
EndFunction

String Function TestFinders()
    Actor dead = Spawn(NakedBase, 100.0, 100.0)
    Actor live = Spawn(NakedBase, 600.0, 600.0)
    dead.Kill(None)
    Utility.Wait(1.0)
    Actor found = Game.FindClosestActorFromRef(CellCenter, 2000.0)
    String r = "dead NPC 141 off, living 849 off: FindClosestActorFromRef gives "
    If found == dead
        r += "the dead"
    ElseIf found == live
        r += "the living"
    ElseIf found == Player
        r += "the player"
    Else
        r += Id(found)
    EndIf
    r += "\n"
    GameHour.SetValue(12.0)
    Utility.Wait(0.5)
    r += "sun at 12: " + Game.GetSunPositionX() + ", " + Game.GetSunPositionY() + ", " + Game.GetSunPositionZ()
    GameHour.SetValue(18.0)
    Utility.Wait(0.5)
    r += "; at 18: " + Game.GetSunPositionX() + ", " + Game.GetSunPositionY() + ", " + Game.GetSunPositionZ() + "\n"
    r += "GameTimeToString(0) " + Utility.GameTimeToString(0.0) + ", (1.5) " + Utility.GameTimeToString(1.5) + "\n"
    r += "missing string setting: [" + Game.GetGameSettingString("sVrmNoSuchSetting") + "]\n"
    Gone(dead)
    Gone(live)
    return r
EndFunction

; actor-natives
String Function TestActorNatives()
    String r = "GetRace: leveled traits template base " + Id(LvlTraitsChild.GetRace()) + " (own RNAM Nord " + Id(NordRace) + ")"
    Actor a = Spawn(LvlTraitsChild, 0.0, 300.0)
    r += ", placed " + Id(a.GetRace()) + "\n"
    r += "GetNthLinkedRef: 1 " + Id(LinkA.GetNthLinkedRef(1)) + " 2 " + Id(LinkA.GetNthLinkedRef(2)) + " 3 " + Id(LinkA.GetNthLinkedRef(3))
    r += " 0 " + Id(LinkA.GetNthLinkedRef(0)) + " -1 " + Id(LinkA.GetNthLinkedRef(-1)) + " (A " + Id(LinkA) + ", B " + Id(LinkB) + ", C " + Id(LinkC) + ")\n"
    r += "keyword-only link: GetLinkedRef() " + Id(LinkK.GetLinkedRef(None)) + ", GetNthLinkedRef(1) " + Id(LinkK.GetNthLinkedRef(1)) + ", with keyword " + Id(LinkK.GetLinkedRef(LinkKeyword)) + "\n"
    Gone(a)
    return r
EndFunction

; EnableAI(false): what an actor still does.
String Function TestEnableAI()
    Actor a = Spawn(LoggedBase, 0.0, 300.0)
    Utility.Wait(1.0)
    (a as VrmAt21ActorLog).Take()
    a.RegisterForAnimationEvent(a, "staggerStart")
    a.EnableAI(false)
    Float x = a.GetPositionX()
    a.StartCombat(Player)
    Utility.Wait(2.0)
    String r = "AI off, StartCombat(player): combat state " + a.GetCombatState() + ", in combat " + a.IsInCombat()
    a.DamageActorValue("Health", 5.0)
    WhFireSpell.Cast(Player, a)
    Utility.Wait(2.0)
    r += "; a fire spell at it: events " + (a as VrmAt21ActorLog).Take() + ", moved " + Num(a.GetPositionX() - x)
    r += ", detects the player " + Player.IsDetectedBy(a) + "\n"
    a.EnableAI(true)
    a.StopCombat()
    Utility.Wait(1.0)
    Gone(a)
    return r
EndFunction

; created-refs
String Function TestDelete()
    ObjectReference o = Put(Barrel, 0.0, 400.0, 0.0)
    o.Delete()
    Utility.Wait(1.0)
    String r = "created barrel held in a variable, Delete: IsDeleted " + o.IsDeleted() + ", Is3DLoaded " + o.Is3DLoaded() + ", disabled " + o.IsDisabled() + ", x " + Num(o.GetPositionX()) + "\n"
    DeleteMe.Delete()
    Utility.Wait(1.0)
    r += "plugin reference, Delete: IsDeleted " + DeleteMe.IsDeleted() + ", disabled " + DeleteMe.IsDisabled() + ", Is3DLoaded " + DeleteMe.Is3DLoaded() + "\n"
    ObjectReference c = Put(EmptyChest, 200.0, 0.0, 0.0)
    ObjectReference s = Put(IronSword, 0.0, 200.0, 50.0)
    c.AddItem(s, 1, true)
    Utility.Wait(0.5)
    r += "a created sword put in a chest: count " + c.GetItemCount(IronSword) + ", sword's container x " + Num(s.GetPositionX())
    s.Delete()
    Utility.Wait(0.5)
    r += "; Delete the sword: count " + c.GetItemCount(IronSword) + "\n"
    Gone(c)
    return r
EndFunction

; outfits
String Function TestOutfits()
    Actor a = Spawn(OutfitBase, 0.0, 300.0)
    Actor b = Spawn(OutfitBase, 300.0, 300.0)
    Utility.Wait(1.0)
    a.AddItem(IronCuirass, 1, true)
    String r = "wearing outfit A (iron cuirass), given another: cuirasses " + a.GetItemCount(IronCuirass) + ", worn " + a.IsEquipped(IronCuirass)
    a.SetOutfit(OutfitB, false)
    Utility.Wait(1.5)
    r += "; Actor.SetOutfit(B, leather): iron " + a.GetItemCount(IronCuirass) + ", leather " + a.GetItemCount(LeatherCuirass) + ", leather worn " + a.IsEquipped(LeatherCuirass)
    r += "; the other of its base: iron " + b.GetItemCount(IronCuirass) + ", leather " + b.GetItemCount(LeatherCuirass) + "\n"
    a.SetOutfit(OutfitA, false)
    Utility.Wait(1.5)
    r += "back to A: iron " + a.GetItemCount(IronCuirass) + ", leather " + a.GetItemCount(LeatherCuirass) + "\n"
    Gone(a)
    Gone(b)
    return r
EndFunction

; formlists
String Function TestFormLists()
    String r = "plugin list [iron sword, steel sword]: size " + TwoSwords.GetSize()
    TwoSwords.AddForm(Gold)
    r += "; AddForm(gold): size " + TwoSwords.GetSize() + ", at 0 " + Id(TwoSwords.GetAt(0)) + ", at 2 " + Id(TwoSwords.GetAt(2)) + ", Find(gold) " + TwoSwords.Find(Gold)
    TwoSwords.AddForm(IronSword)
    r += "; AddForm(iron sword) again: size " + TwoSwords.GetSize()
    TwoSwords.AddForm(Gold)
    r += "; AddForm(gold) again: size " + TwoSwords.GetSize()
    TwoSwords.RemoveAddedForm(SteelSword)
    r += "; RemoveAddedForm(steel sword, the plugin's): has it " + TwoSwords.HasForm(SteelSword) + ", size " + TwoSwords.GetSize()
    TwoSwords.AddForm(IngotIron)
    r += "; AddForm(ingot): at 0 " + Id(TwoSwords.GetAt(0)) + ", at 3 " + Id(TwoSwords.GetAt(3))
    TwoSwords.Revert()
    r += "; Revert: size " + TwoSwords.GetSize() + "\n"
    r += "(iron sword " + Id(IronSword) + ", steel " + Id(SteelSword) + ", gold " + Id(Gold) + ", ingot " + Id(IngotIron) + ")\n"
    return r
EndFunction

; factions: reactions, expulsion, crime groups.
String Function TestFactionNatives()
    Actor a = Spawn(RedBase, 0.0, 300.0)
    Actor b = Spawn(GreenBase, 300.0, 300.0)
    a.SetDontMove(true)
    b.SetDontMove(true)
    Utility.Wait(1.0)
    String r = "Red-Green: GetReaction " + RedFaction.GetReaction(GreenFaction) + "/" + GreenFaction.GetReaction(RedFaction) + ", hostile " + a.IsHostileToActor(b)
    RedFaction.SetReaction(GreenFaction, 1)
    Utility.Wait(1.0)
    r += "; SetReaction(1): GetReaction " + RedFaction.GetReaction(GreenFaction) + ", hostile " + a.IsHostileToActor(b) + "/" + b.IsHostileToActor(a) + ", fighting " + a.GetCombatState()
    RedFaction.SetReaction(GreenFaction, 0)
    RedFaction.SetEnemy(GreenFaction, false, false)
    Utility.Wait(1.0)
    r += "; SetEnemy: GetReaction " + RedFaction.GetReaction(GreenFaction) + ", hostile " + a.IsHostileToActor(b) + "\n"
    RedFaction.SetAlly(GreenFaction, false, false)
    Player.SetFactionRank(GreenFaction, 2)
    GreenFaction.SetPlayerExpelled(true)
    r += "player rank 2 in Green, SetPlayerExpelled: in faction " + Player.IsInFaction(GreenFaction) + ", rank " + Player.GetFactionRank(GreenFaction) + ", IsPlayerExpelled " + GreenFaction.IsPlayerExpelled() + "\n"
    GreenFaction.SetPlayerExpelled(false)
    Player.RemoveFromFaction(GreenFaction)
    r += "crime groups: Red in its own unlisted " + RedFaction.IsFactionInCrimeGroup(RedFaction)
    r += ", Grouped (lists Red) has Red " + GroupedFaction.IsFactionInCrimeGroup(RedFaction) + ", Red has Grouped " + RedFaction.IsFactionInCrimeGroup(GroupedFaction)
    r += ", Grouped in its own (unlisted) " + GroupedFaction.IsFactionInCrimeGroup(GroupedFaction) + "\n"
    Gone(a)
    Gone(b)
    return r
EndFunction

; protection: base flags, ghosts, invulnerable.
String Function TestProtection()
    Actor a = Spawn(ProtectBase, 0.0, 300.0)
    Utility.Wait(1.0)
    ProtectBase.SetEssential(true)
    String r = "base SetEssential while loaded: IsEssential " + a.IsEssential()
    a.Kill(None)
    Utility.Wait(2.0)
    r += ", Kill: dead " + a.IsDead() + ", bleeding out " + a.IsBleedingOut() + "\n"
    ProtectBase.SetEssential(false)
    a.Resurrect()
    Utility.Wait(1.0)
    Actor g = Spawn(NakedBase, 300.0, 300.0)
    g.SetGhost(true)
    Float h = g.GetActorValue("Health")
    g.DamageActorValue("Health", 10.0)
    r += "ghost: IsGhost " + g.IsGhost() + ", DamageActorValue(10) took " + Num(h - g.GetActorValue("Health"))
    h = g.GetActorValue("Health")
    WhFireSpell.Cast(Player, g)
    Utility.Wait(2.0)
    r += ", a fire spell took " + Num(h - g.GetActorValue("Health")) + "\n"
    Actor v = Spawn(InvulnerableBase, -300.0, 300.0)
    Utility.Wait(0.5)
    h = v.GetActorValue("Health")
    Float st = v.GetActorValue("Stamina")
    v.DamageActorValue("Health", 10.0)
    v.DamageActorValue("Stamina", 10.0)
    r += "invulnerable base: Damage health 10 took " + Num(h - v.GetActorValue("Health")) + ", stamina 10 took " + Num(st - v.GetActorValue("Stamina"))
    h = v.GetActorValue("Health")
    v.ForceActorValue("Health", 1.0)
    r += ", ForceActorValue(1) gives " + Num(v.GetActorValue("Health"))
    v.RestoreActorValue("Health", 1000.0)
    WhFireSpell.Cast(Player, v)
    Utility.Wait(2.0)
    r += ", a fire spell took " + Num(v.GetActorValueMax("Health") - v.GetActorValue("Health")) + "\n"
    Gone(a)
    Gone(g)
    Gone(v)
    return r
EndFunction

; protection: quest aliases' essential and protected flags.
String Function TestAliasProtection()
    Actor a = Spawn(NakedBase, 0.0, 300.0)
    Utility.Wait(0.5)
    String r = "before: essential " + a.IsEssential()
    AliasQuest.Start()
    Utility.Wait(0.5)
    ReferenceAlias al = AliasQuest.GetAlias(0) as ReferenceAlias
    al.ForceRefTo(a)
    Utility.Wait(0.5)
    r += "; in an essential alias: essential " + a.IsEssential()
    al.Clear()
    Utility.Wait(0.5)
    r += "; alias cleared, quest running: essential " + a.IsEssential()
    al.ForceRefTo(a)
    AliasQuest.Stop()
    Utility.Wait(1.0)
    r += "; refilled, quest stopped: essential " + a.IsEssential() + "\n"
    Gone(a)
    return r
EndFunction

; crime.md: runtime faction changes on an NPC outside the faction.
String Function TestFactionRanks()
    Actor a = Spawn(NakedBase, 0.0, 300.0)
    String r = "out: rank " + a.GetFactionRank(GreenFaction) + ", in " + a.IsInFaction(GreenFaction)
    a.ModFactionRank(GreenFaction, 2)
    r += "; ModFactionRank(+2) while out: rank " + a.GetFactionRank(GreenFaction) + ", in " + a.IsInFaction(GreenFaction)
    a.AddToFaction(GreenFaction)
    r += "; AddToFaction: rank " + a.GetFactionRank(GreenFaction) + ", in " + a.IsInFaction(GreenFaction)
    a.SetFactionRank(GreenFaction, 3)
    a.AddToFaction(GreenFaction)
    r += "; rank 3, AddToFaction again: rank " + a.GetFactionRank(GreenFaction)
    a.ModFactionRank(GreenFaction, -5)
    r += "; ModFactionRank(-5): rank " + a.GetFactionRank(GreenFaction) + ", in " + a.IsInFaction(GreenFaction)
    a.RemoveFromFaction(GreenFaction)
    r += "; RemoveFromFaction: rank " + a.GetFactionRank(GreenFaction) + ", in " + a.IsInFaction(GreenFaction)
    a.SetFactionRank(GreenFaction, 1)
    r += "; SetFactionRank(1) while out: rank " + a.GetFactionRank(GreenFaction) + ", in " + a.IsInFaction(GreenFaction)
    a.SetFactionRank(GreenFaction, -1)
    r += "; SetFactionRank(-1): rank " + a.GetFactionRank(GreenFaction) + ", in " + a.IsInFaction(GreenFaction) + "\n"
    Actor g = Spawn(GuardClassBase, 300.0, 300.0)
    Actor f = Spawn(GuardFactionBase, -300.0, 300.0)
    r += "IsGuard: guard class only " + g.IsGuard() + ", IsGuardFaction member only " + f.IsGuard() + "\n"
    Gone(a)
    Gone(g)
    Gone(f)
    return r
EndFunction

; crime.md: infamy after paying; GetCrimeGold of a faction never wronged.
String Function TestInfamy()
    String r = "crime faction: gold " + CrimeFaction.GetCrimeGold() + ", infamy " + CrimeFaction.GetInfamy()
    CrimeFaction.ModCrimeGold(100, false)
    CrimeFaction.ModCrimeGold(40, true)
    r += "; +100 non-violent, +40 violent: gold " + CrimeFaction.GetCrimeGold() + " (violent " + CrimeFaction.GetCrimeGoldViolent() + "), infamy " + CrimeFaction.GetInfamy() + " (violent " + CrimeFaction.GetInfamyViolent() + ")"
    Int had = Player.GetItemCount(Gold)
    Player.AddItem(Gold, 500, true)
    CrimeFaction.PlayerPayCrimeGold(false, false)
    r += "; paid: gold " + CrimeFaction.GetCrimeGold() + ", infamy " + CrimeFaction.GetInfamy() + ", paid " + (had + 500 - Player.GetItemCount(Gold))
    CrimeFaction.SetCrimeGold(50)
    r += "; SetCrimeGold(50): infamy " + CrimeFaction.GetInfamy() + "\n"
    CrimeFaction.SetCrimeGold(0)
    Player.RemoveItem(Gold, Player.GetItemCount(Gold) - had, true, None)
    return r
EndFunction
