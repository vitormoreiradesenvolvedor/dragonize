// Headless: decompile functions to a C file.
// args: outFile (addr[,addr...] | all | calls:EXTERNAL_NAME[,NAME...] | str:SUBSTRING)
//   calls:  every function calling one of the named imports (e.g. calls:HidD_SetFeature)
//   callers:ADDR[,ADDR]  every function referencing one of the addresses
//   str:    every function referencing a string containing SUBSTRING
import ghidra.app.decompiler.*;
import ghidra.app.script.GhidraScript;
import ghidra.program.model.address.Address;
import ghidra.program.model.listing.*;
import ghidra.program.model.symbol.*;
import java.io.*;
import java.util.*;

public class Decomp extends GhidraScript {
    @Override
    public void run() throws Exception {
        String[] a = getScriptArgs();
        PrintWriter out = new PrintWriter(new FileWriter(a[0]));
        TreeSet<Function> fs = new TreeSet<>(Comparator.comparing(Function::getEntryPoint));
        String spec = a[1];
        if (spec.equals("all")) {
            for (Function f : currentProgram.getListing().getFunctions(true)) fs.add(f);
        } else if (spec.startsWith("calls:")) {
            Set<String> names = new HashSet<>(Arrays.asList(spec.substring(6).split(",")));
            for (Function f : currentProgram.getFunctionManager().getExternalFunctions()) {
                if (!names.contains(f.getName())) continue;
                Set<Address> targets = new HashSet<>();
                targets.add(f.getEntryPoint());
                for (Address th : f.getFunctionThunkAddresses(true) == null ? new Address[0] : f.getFunctionThunkAddresses(true)) targets.add(th);
                for (Reference r : getReferencesTo(f.getEntryPoint())) targets.add(r.getFromAddress());
                for (Address t : targets) {
                    for (Reference r : getReferencesTo(t)) {
                        Function c = getFunctionContaining(r.getFromAddress());
                        if (c != null) fs.add(c);
                    }
                }
            }
        } else if (spec.startsWith("callers:")) {
            for (String s : spec.substring(8).split(",")) {
                Address t = toAddr(Long.parseLong(s, 16));
                for (Reference r : getReferencesTo(t)) {
                    Function c = getFunctionContaining(r.getFromAddress());
                    if (c != null) fs.add(c);
                }
            }
        } else if (spec.startsWith("str:")) {
            String sub = spec.substring(4);
            DataIterator it = currentProgram.getListing().getDefinedData(true);
            while (it.hasNext()) {
                Data d = it.next();
                if (!d.hasStringValue() || !String.valueOf(d.getValue()).contains(sub)) continue;
                for (Reference r : getReferencesTo(d.getAddress())) {
                    Function c = getFunctionContaining(r.getFromAddress());
                    if (c != null) fs.add(c);
                }
            }
        } else {
            for (String s : spec.split(",")) {
                Function f = getFunctionContaining(toAddr(Long.parseLong(s, 16)));
                if (f == null) {
                    disassemble(toAddr(Long.parseLong(s, 16)));
                    f = createFunction(toAddr(Long.parseLong(s, 16)), null);
                }
                if (f != null) fs.add(f);
                else out.println("// no function at " + s);
            }
        }
        DecompInterface di = new DecompInterface();
        di.openProgram(currentProgram);
        for (Function f : fs) {
            if (monitor.isCancelled()) break;
            DecompileResults r = di.decompileFunction(f, 120, monitor);
            out.println("// ===== " + f.getName() + " @ " + f.getEntryPoint());
            out.println(r.decompileCompleted() ? r.getDecompiledFunction().getC() : "// failed: " + r.getErrorMessage());
        }
        out.close();
    }
}
