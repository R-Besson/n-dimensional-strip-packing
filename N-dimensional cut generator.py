import sys, itertools

# constructive solid geometry cut developed for N dimensions
def generate_csg_code(n):
	origin = [1]
	dims = ['x','y','z']+[f'd{i}' for i in range(3, n)]
	
	box_prefix = "box.position"
	hole_prefix = "hole.position"
	
	def get_dim_cases(s):
		return [
			([f"{hole_prefix}.{s}", f"{hole_prefix}.{s}2()"], f"{box_prefix}.{s}<={hole_prefix}.{s} && {box_prefix}.{s}2() >= {hole_prefix}.{s}2()", {s:0,f"{s}2()":2}),
			([f"{hole_prefix}.{s}",f"{box_prefix}.{s}2()",f"{hole_prefix}.{s}2()"], f"{box_prefix}.{s} <= {hole_prefix}.{s} && {box_prefix}.{s}2() < {hole_prefix}.{s}2()", {s:-1,f"{s}2()":1}),
			([f"{hole_prefix}.{s}",f"{box_prefix}.{s}",f"{hole_prefix}.{s}2()"], f"{box_prefix}.{s} > {hole_prefix}.{s} && {box_prefix}.{s}2() >= {hole_prefix}.{s}2()", {s:1,f"{s}2()":3}),
			([f"{hole_prefix}.{s}",f"{box_prefix}.{s}",f"{box_prefix}.{s}2()",f"{hole_prefix}.{s}2()"], f"{box_prefix}.{s} > {hole_prefix}.{s} && {box_prefix}.{s}2() < {hole_prefix}.{s}2()", {s:0.5,f"{s}2()":1.5})
		]

	all_dim_cases = [get_dim_cases(d) for d in dims]
	is_first_case = [True]

	for p_config in itertools.product(*all_dim_cases):
		rect_vals = {}
		for p in p_config: rect_vals.update(p[2])
		hole_vals = {k: v for d in dims for k, v in {d: 0, f'{d}2()': 2}.items()}
		
		conditions = " && \n    ".join([f"({p[1]})" for p in p_config])
		print(f"{'if' if is_first_case[0] else 'else if'} (\n    {conditions}\n) {{")
		is_first_case[0] = False

		potential_subs = []
		cut_points = [p[0] for p in p_config]
		
		index_pairs_per_dim = [list(itertools.combinations(range(len(c)), 2)) for c in cut_points]

		for index_combo in itertools.product(*index_pairs_per_dim):
			new_box_vals = {}
			min_names, max_names = [], []
			for i, d in enumerate(dims):
				min_idx, max_idx = index_combo[i]
				min_n, max_n = cut_points[i][min_idx], cut_points[i][max_idx]
				min_names.append(min_n); max_names.append(max_n)
				new_box_vals[d] = hole_vals[min_n[len(hole_prefix)+1:]] if 'h'==min_n[0] else rect_vals[min_n[len(box_prefix)+1:]]
				new_box_vals[f'{d}2()'] = hole_vals[max_n[len(hole_prefix)+1:]] if 'h'==max_n[0] else rect_vals[max_n[len(box_prefix)+1:]]

			intersects = all((new_box_vals[f'{d}2()']>rect_vals[d] and rect_vals[f'{d}2()']>new_box_vals[d]) for d in dims)
			is_in = all((new_box_vals[d]>=hole_vals[d] and new_box_vals[f'{d}2()']<=hole_vals[f'{d}2()']) for d in dims)

			if not intersects and is_in:
				potential_subs.append({'vals': new_box_vals, 'min': min_names, 'max': max_names})
		
		final_subs = []
		for new_sub in potential_subs:
			is_enveloped = any([all(new_sub['vals'][d] >= ex['vals'][d] and new_sub['vals'][f'{d}2()'] <= ex['vals'][f'{d}2()'] for d in dims) for ex in final_subs])
			if not is_enveloped:
				final_subs = [ex for ex in final_subs if not all(ex['vals'][d] >= new_sub['vals'][d] and ex['vals'][f'{d}2()'] <= new_sub['vals'][f'{d}2()'] for d in dims)]
				final_subs.append(new_sub)

		for sub in final_subs:
			starts = ",".join(sub['min'])
			sizes = ",".join(f"{ma}-{mi}" for mi, ma in zip(sub['min'], sub['max']))
			print(f"	*next_hole_id += 1; new_holes.push(Box3D::from_xyz_whl({starts},{sizes},*next_hole_id,{origin[0]}));")
			origin[0] += 1
		print("}")

if __name__ == "__main__":
	if len(sys.argv) > 1:
		generate_csg_code(int(sys.argv[1]))
	else:
		print("Please specify an integer dimension N")